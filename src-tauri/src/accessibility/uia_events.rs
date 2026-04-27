use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uiautomation::patterns::{UITextPattern, UIValuePattern};
use uiautomation::UIAutomation;
use windows::Win32::System::Threading::{GetCurrentProcessId, Sleep};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

/// Minimum text length to emit as a selection event.
/// Filters out noise from single-click, keyboard navigation, etc.
const MIN_SELECTION_LEN: usize = 3;

/// Event payload sent to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiaTextEvent {
    pub text: String,
    pub app_name: String,
    pub window_title: String,
    /// "focus-changed" (log only) or "selection-changed" (triggers translation).
    pub event_type: String,
    /// "event" (UIA COM event) or "polling" (fallback polling).
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_source() -> String {
    "event".to_string()
}

/// Control handle for the UIA event listener thread.
pub struct UiaEventListener {
    running: AtomicBool,
    stop_tx: std::sync::Mutex<Option<mpsc::Sender<()>>>,
}

impl UiaEventListener {
    const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            stop_tx: std::sync::Mutex::new(None),
        }
    }

    /// Start the event listener thread. No-op if already running.
    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            log::info!("UIA Events: 已在运行，跳过");
            return Ok(());
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        // Channel for handler → bridge thread communication.
        let (event_tx, event_rx) = mpsc::channel::<UiaTextEvent>();

        // Spawn threads BEFORE setting running flag.
        // This eliminates the window where threads exist but stop_tx is not yet assigned.
        thread::Builder::new()
            .name("uia-event-listener".into())
            .spawn(move || {
                if let Err(e) = listener_thread_main(event_tx, stop_rx) {
                    log::error!("UIA Events 监听线程错误: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn listener thread: {}", e))?;

        thread::Builder::new()
            .name("uia-event-bridge".into())
            .spawn(move || {
                while let Ok(event) = event_rx.recv() {
                    let _ = app.emit("uia-text-event", &event);
                }
                log::debug!("UIA Events: Bridge 线程退出");
            })
            .map_err(|e| format!("Failed to spawn bridge thread: {}", e))?;

        // Set running and store stop_tx AFTER threads are spawned.
        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(stop_tx);
        } else {
            log::error!("UIA Events: stop_tx Mutex poisoned");
            return Err("Internal lock error".to_string());
        }
        self.running.store(true, Ordering::SeqCst);

        log::info!("UIA Events 监听器已启动");
        Ok(())
    }

    /// Stop the event listener thread. No-op if not running.
    pub fn stop(&self) -> Result<(), String> {
        if !self.running.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        // Send stop signal. Take the sender so it's dropped, closing the channel.
        let tx = self.stop_tx.lock().ok().and_then(|mut guard| guard.take());
        if let Some(tx) = tx {
            let _ = tx.send(());
        }

        log::info!("UIA Events: 停止信号已发送");
        Ok(())
    }

    /// Check if the listener is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Main loop for the listener thread.
///
/// Initializes COM (MTA) and UIAutomation, registers event handlers,
/// then pumps Windows messages to receive COM callbacks.
fn listener_thread_main(
    event_tx: mpsc::Sender<UiaTextEvent>,
    stop_rx: mpsc::Receiver<()>,
) -> Result<(), String> {
    // Initialize UIAutomation (internally calls CoInitializeEx with MTA).
    let automation = UIAutomation::new()
        .map_err(|e| format!("UIAutomation init failed: {}", e))?;

    log::info!("UIA Events: UIAutomation 已初始化");

    // --- Phase 2: Register focus-changed handler (log only) ---
    let focus_handler = create_focus_changed_handler(event_tx.clone());
    if let Err(e) = automation.add_focus_changed_event_handler(None, &focus_handler) {
        log::error!("UIA Events: 添加焦点处理器失败: {}", e);
    } else {
        log::info!("UIA Events: 焦点处理器已注册");
    }

    // --- Phase 3: Register selection-changed handler (triggers translation) ---
    let selection_handler = create_selection_changed_handler(event_tx);
    let desktop = automation.get_root_element()
        .map_err(|e| format!("Get root element failed: {}", e))?;
    if let Err(e) = automation.add_automation_event_handler(
        uiautomation::events::UIEventType::Text_TextSelectionChanged,
        &desktop,
        uiautomation::types::TreeScope::Subtree,
        None,
        &selection_handler,
    ) {
        log::error!("UIA Events: 添加选择处理器失败: {}", e);
    } else {
        log::info!("UIA Events: 选择处理器已注册");
    }

    // Message pump: process COM callbacks until stop signal.
    let mut msg = MSG::default();
    loop {
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Check stop signal (non-blocking).
        match stop_rx.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => {
                log::info!("UIA Events: 停止信号已收到");
                break;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Avoid busy-spinning.
        unsafe { Sleep(50) };
    }

    // Cleanup: remove handlers.
    let _ = automation.remove_focus_changed_event_handler(&focus_handler);
    let _ = automation.remove_automation_event_handler(
        uiautomation::events::UIEventType::Text_TextSelectionChanged,
        &desktop,
        &selection_handler,
    );

    log::info!("UIA Events: 监听线程已退出");
    Ok(())
}

// === Handler factories (stubs, filled in Phase 2/3) ===

// Thread-local UIAutomation instance to avoid per-event creation.
//
// The SelectionHandler runs on the listener thread on every COM callback.
// Creating a new `UIAutomation` per event is wasteful (~COM object alloc).
// `thread_local!` with `RefCell` provides single-thread caching; the `RefCell`
// borrow is held only for the duration of `get_top_level_window_title`.
thread_local! {
    static THREAD_AUTOMATION: std::cell::RefCell<Option<UIAutomation>> =
        std::cell::RefCell::new(None);
}

/// Get or create a thread-local UIAutomation instance.
/// Returns None if creation fails.
fn with_thread_automation<R>(f: impl FnOnce(&UIAutomation) -> R) -> Option<R> {
    THREAD_AUTOMATION.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if borrow.is_none() {
            match UIAutomation::new() {
                Ok(a) => *borrow = Some(a),
                Err(e) => {
                    log::error!("UIA Events: thread-local UIAutomation 创建失败: {}", e);
                    return None;
                }
            }
        }
        // Safety: borrow is Some at this point.
        Some(f(borrow.as_ref().unwrap()))
    })
}

fn create_focus_changed_handler(
    _event_tx: mpsc::Sender<UiaTextEvent>,
) -> uiautomation::events::UIFocusChangedEventHandler {
    // Phase 2: implement focus-changed logging
    struct FocusChangedLog;
    impl uiautomation::events::CustomFocusChangedEventHandler for FocusChangedLog {
        fn handle(&self, sender: &uiautomation::UIElement) -> uiautomation::Result<()> {
            let name = sender.get_name().unwrap_or_default();
            let class = sender.get_classname().unwrap_or_default();
            log::trace!("UIA Events 焦点变化: name={name:?}, class={class:?}");
            Ok(())
        }
    }
    uiautomation::events::UIFocusChangedEventHandler::from(FocusChangedLog)
}

fn create_selection_changed_handler(
    event_tx: mpsc::Sender<UiaTextEvent>,
) -> uiautomation::events::UIEventHandler {
    struct SelectionHandler {
        tx: mpsc::Sender<UiaTextEvent>,
        // Dedup state — handler is bound to the listener thread, no cross-thread sharing.
        last_text: std::cell::RefCell<String>,
        last_emit: std::cell::RefCell<Instant>,
    }

    impl uiautomation::events::CustomEventHandler for SelectionHandler {
        fn handle(
            &self,
            sender: &uiautomation::UIElement,
            event_type: uiautomation::events::UIEventType,
        ) -> uiautomation::Result<()> {
            if event_type != uiautomation::events::UIEventType::Text_TextSelectionChanged {
                return Ok(());
            }

            // Skip Tauri's own windows.
            if is_tauri_window(sender) {
                return Ok(());
            }

            // Extract selected text.
            let text = extract_selected_text(sender);
            if text.is_empty() {
                return Ok(());
            }

            // Filter noise: single chars, whitespace-only, etc.
            if text.trim().len() < MIN_SELECTION_LEN {
                return Ok(());
            }

            // Dedup: same text within 500ms.
            {
                let mut last = self.last_text.borrow_mut();
                let mut last_time = self.last_emit.borrow_mut();
                let now = Instant::now();
                if *last == text && now.duration_since(*last_time) < Duration::from_millis(500) {
                    return Ok(());
                }
                *last = text.clone();
                *last_time = now;
            }

            let app_name = sender
                .get_process_id()
                .map(crate::accessibility::process_name_from_pid)
                .unwrap_or_default();
            let window_title = with_thread_automation(|a| get_top_level_window_title(sender, a))
                .unwrap_or_else(|| sender.get_name().unwrap_or_default());

            // 黑名单过滤（进程名精确 + 窗口标题子串，不区分大小写）
            if crate::accessibility::is_blocked(&app_name, &window_title) {
                return Ok(());
            }

            log::debug!(
                "UIA Events 选择变化: {} chars from {:?}",
                text.len(),
                app_name
            );

            // Update shared timestamp for polling fallback coordination.
            crate::accessibility::uia_polling::LAST_SELECTION_EVENT_AT.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                std::sync::atomic::Ordering::Release,
            );

            let _ = self.tx.send(UiaTextEvent {
                text,
                app_name,
                window_title,
                event_type: "selection-changed".to_string(),
                source: "event".to_string(),
            });

            Ok(())
        }
    }

    uiautomation::events::UIEventHandler::from(SelectionHandler {
        tx: event_tx,
        last_text: std::cell::RefCell::new(String::new()),
        last_emit: std::cell::RefCell::new(Instant::now()),
    })
}

// === Helpers ===

/// Maximum characters to read from a text range.
const MAX_SELECTION_CHARS: i32 = 50_000;

/// Try to extract selected text from an element via TextPattern, then ValuePattern.
fn extract_selected_text(element: &uiautomation::UIElement) -> String {
    // Try TextPattern first (supports partial selection).
    if let Ok(pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(selections) = pattern.get_selection() {
            let texts: Vec<String> = selections
                .iter()
                .filter_map(|range| range.get_text(MAX_SELECTION_CHARS).ok())
                .filter(|t| !t.is_empty())
                .collect();
            if !texts.is_empty() {
                return texts.join(" ");
            }
        }
    }

    // Fall back to ValuePattern (full value, not selection).
    if let Ok(pattern) = element.get_pattern::<UIValuePattern>() {
        if let Ok(value) = pattern.get_value() {
            if !value.is_empty() {
                return value;
            }
        }
    }

    String::new()
}

/// Check if the element belongs to the current (Tauri) process.
///
/// Uses process ID comparison — the most reliable method.
/// Falls back to class name matching only if PID is unavailable.
fn is_tauri_window(element: &uiautomation::UIElement) -> bool {
    // Primary: compare process IDs.
    if let Ok(elem_pid) = element.get_process_id() {
        let my_pid = unsafe { GetCurrentProcessId() };
        return elem_pid == my_pid;
    }

    // Fallback: class name heuristic (less reliable, but covers edge cases).
    let class = element.get_classname().unwrap_or_default();
    class.contains("WebView") || class.contains("Tauri")
}

/// Get the window title by traversing up to the top-level Window element.
/// Falls back to the element's own name if traversal fails.
fn get_top_level_window_title(
    element: &uiautomation::UIElement,
    automation: &UIAutomation,
) -> String {
    if let Ok(walker) = automation.get_control_view_walker() {
        let mut current = element.clone();
        for _ in 0..20 {
            match walker.get_parent(&current) {
                Ok(parent) => {
                    let ctrl_type = parent.get_control_type();
                    if ctrl_type == Ok(uiautomation::types::ControlType::Window) {
                        return parent.get_name().unwrap_or_default();
                    }
                    current = parent;
                }
                Err(_) => break,
            }
        }
    }

    element.get_name().unwrap_or_default()
}

// === Singleton ===

static LISTENER: OnceLock<UiaEventListener> = OnceLock::new();

pub fn get_listener() -> &'static UiaEventListener {
    LISTENER.get_or_init(|| UiaEventListener::new())
}
