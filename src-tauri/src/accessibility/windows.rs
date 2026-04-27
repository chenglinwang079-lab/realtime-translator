use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use uiautomation::patterns::UITextPattern;
use uiautomation::patterns::UIValuePattern;
use uiautomation::types::ControlType;
use uiautomation::{UIAutomation, UIElement};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, GetMessageW, IsWindow,
    MSG, EVENT_SYSTEM_FOREGROUND,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

/// Track the last non-Tauri foreground window so we can read selected text
/// even after the Tauri window steals focus (e.g. when user clicks the grab button).
static LAST_FOREGROUND: AtomicIsize = AtomicIsize::new(0);

use super::{AccessibilityWatcher, TextSelection};

/// Commands sent to the STA thread.
enum UiaRequest {
    GetSelectedText,
    GetFocusedAppName,
    Shutdown,
}

/// Response from the STA thread.
enum UiaResponse {
    SelectedText(Option<TextSelection>),
    AppName(String),
    Error(String),
}

/// Persistent STA thread that owns the UIAutomation instance.
struct UiaStaThread {
    tx: mpsc::Sender<UiaRequest>,
    rx: mpsc::Receiver<UiaResponse>,
    handle: Option<thread::JoinHandle<()>>,
}

impl UiaStaThread {
    fn new() -> Result<Self> {
        let (req_tx, req_rx) = mpsc::channel::<UiaRequest>();
        let (resp_tx, resp_rx) = mpsc::channel::<UiaResponse>();

        let handle = thread::Builder::new()
            .name("uia-sta".into())
            .spawn(move || {
            let automation = match UIAutomation::new() {
                Ok(a) => a,
                Err(e) => {
                    log::error!("STA thread init failed: {}", e);
                    let _ = resp_tx.send(UiaResponse::Error(e.to_string()));
                    return;
                }
            };

            log::info!("STA thread initialized successfully");

            loop {
                match req_rx.recv() {
                    Ok(UiaRequest::GetSelectedText) => {
                        let result = get_selected_text_impl(&automation);
                        let _ = resp_tx.send(result);
                    }
                    Ok(UiaRequest::GetFocusedAppName) => {
                        let result = get_focused_app_impl(&automation);
                        let _ = resp_tx.send(result);
                    }
                    Ok(UiaRequest::Shutdown) | Err(_) => {
                        log::info!("STA thread shutting down");
                        break;
                    }
                }
            }
        })?;

        Ok(Self {
            tx: req_tx,
            rx: resp_rx,
            handle: Some(handle),
        })
    }

    fn get_selected_text(&self) -> Result<Option<TextSelection>> {
        self.tx.send(UiaRequest::GetSelectedText)
            .map_err(|_| anyhow::anyhow!("UIA STA thread disconnected"))?;
        match self.rx.recv() {
            Ok(UiaResponse::SelectedText(text)) => Ok(text),
            Ok(UiaResponse::Error(e)) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Unexpected UIA response")),
        }
    }

    fn get_focused_app_name(&self) -> Result<String> {
        self.tx.send(UiaRequest::GetFocusedAppName)
            .map_err(|_| anyhow::anyhow!("UIA STA thread disconnected"))?;
        match self.rx.recv() {
            Ok(UiaResponse::AppName(name)) => Ok(name),
            Ok(UiaResponse::Error(e)) => Err(anyhow::anyhow!(e)),
            _ => Err(anyhow::anyhow!("Unexpected UIA response")),
        }
    }
}

impl Drop for UiaStaThread {
    fn drop(&mut self) {
        let _ = self.tx.send(UiaRequest::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// === Functions that run ON the STA thread ===

fn get_selected_text_impl(automation: &UIAutomation) -> UiaResponse {
    let element = match get_foreground_element(automation) {
        Ok(e) => e,
        Err(e) => return UiaResponse::Error(e.to_string()),
    };

    let (window_title, window_class) = get_element_info(&element);
    let app_name = element
        .get_process_id()
        .map(crate::accessibility::process_name_from_pid)
        .unwrap_or_default();
    log::debug!("foreground window: title={window_title:?}, class={window_class:?}, app={app_name:?}");

    // Try TextPattern on the foreground element itself
    if let Ok(Some(text)) = try_text_pattern(&element) {
        log::debug!("found text via TextPattern on foreground element");
        return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &window_class, &window_title)));
    }

    // Try ValuePattern on the foreground element
    if let Ok(Some(text)) = try_value_pattern(&element) {
        log::debug!("found text via ValuePattern on foreground element");
        return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &window_class, &window_title)));
    }

    // Use matcher to find Edit elements within the foreground window
    log::debug!("searching for Edit controls...");
    let matcher = automation.create_matcher()
        .from(element.clone())
        .timeout(1000)
        .control_type(ControlType::Edit);

    if let Ok(found) = matcher.find_first() {
        let (name, class) = get_element_info(&found);
        log::debug!("found Edit: name={name:?}, class={class:?}");

        if let Ok(Some(text)) = try_text_pattern(&found) {
            log::debug!("got text via TextPattern on Edit");
            return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &class, &window_title)));
        }
        if let Ok(Some(text)) = try_value_pattern(&found) {
            log::debug!("got text via ValuePattern on Edit");
            return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &class, &window_title)));
        }
        log::debug!("Edit element has no TextPattern/ValuePattern");
    } else {
        log::debug!("no Edit control found");
    }

    // Try Document control type (used by some apps like Word, Chrome)
    log::debug!("searching for Document controls...");
    let matcher = automation.create_matcher()
        .from(element.clone())
        .timeout(1000)
        .control_type(ControlType::Document);

    if let Ok(found) = matcher.find_first() {
        let (name, class) = get_element_info(&found);
        log::debug!("found Document: name={name:?}, class={class:?}");

        if let Ok(Some(text)) = try_text_pattern(&found) {
            log::debug!("got text via TextPattern on Document");
            return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &class, &window_title)));
        }
        if let Ok(Some(text)) = try_value_pattern(&found) {
            log::debug!("got text via ValuePattern on Document");
            return UiaResponse::SelectedText(Some(make_selection(text, &app_name, &class, &window_title)));
        }
    } else {
        log::debug!("no Document control found");
    }

    log::debug!("no text found in foreground window");

    // Hint for VS Code / Electron editors that need screen reader mode.
    let title_lower = window_title.to_lowercase();
    if title_lower.contains("visual studio code") || title_lower.contains("vs code") {
        log::info!(
            "VS Code 检测到但未获取到文本，可能需启用屏幕阅读器模式 (Shift+Alt+F1)"
        );
    }

    UiaResponse::SelectedText(None)
}

fn get_focused_app_impl(automation: &UIAutomation) -> UiaResponse {
    match get_foreground_element(automation) {
        Ok(element) => UiaResponse::AppName(element.get_name().unwrap_or_default()),
        Err(e) => UiaResponse::Error(e.to_string()),
    }
}

/// Maximum characters to read from a text range.
const MAX_SELECTION_CHARS: i32 = 50_000;

fn get_foreground_element(automation: &UIAutomation) -> Result<UIElement> {
    unsafe {
        let mut hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            anyhow::bail!("No foreground window");
        }

        // If the foreground window is our own Tauri window, fall back to the
        // last non-Tauri window that was tracked by the focus hook.
        let my_pid = GetCurrentProcessId();
        let mut fg_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut fg_pid));
        if fg_pid == my_pid {
            let stored = LAST_FOREGROUND.load(Ordering::Relaxed);
            if stored != 0 {
                let candidate = HWND(stored as *mut _);
                // Validate the stored HWND is still alive
                if IsWindow(Some(candidate)).as_bool() {
                    hwnd = candidate;
                    log::debug!("foreground is Tauri, using last focused window");
                } else {
                    LAST_FOREGROUND.store(0, Ordering::Relaxed);
                    anyhow::bail!("Last focused window no longer exists");
                }
            } else {
                anyhow::bail!("Foreground is Tauri and no previous window tracked");
            }
        }

        let element = automation.element_from_handle(hwnd.into())?;
        Ok(element)
    }
}

fn make_selection(text: String, app_name: &str, window_class: &str, window_title: &str) -> TextSelection {
    TextSelection {
        text,
        app_name: app_name.to_string(),
        window_class: window_class.to_string(),
        window_title: window_title.to_string(),
    }
}

fn try_text_pattern(element: &UIElement) -> Result<Option<String>> {
    let pattern = match element.get_pattern::<UITextPattern>() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    let selections = pattern.get_selection()?;
    if selections.is_empty() {
        return Ok(None);
    }

    let mut texts = Vec::new();
    for range in &selections {
        let text = range.get_text(MAX_SELECTION_CHARS)?;
        if !text.is_empty() {
            texts.push(text);
        }
    }

    if texts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(texts.join(" ")))
    }
}

fn try_value_pattern(element: &UIElement) -> Result<Option<String>> {
    let pattern = match element.get_pattern::<UIValuePattern>() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    let value = pattern.get_value()?;
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

fn get_element_info(element: &UIElement) -> (String, String) {
    let name = element.get_name().unwrap_or_default();
    let class_name = element.get_classname().unwrap_or_default();
    (name, class_name)
}

/// Windows UI Automation implementation of AccessibilityWatcher.
pub struct WindowsUIAWatcher {
    sta: UiaStaThread,
}

impl WindowsUIAWatcher {
    pub fn new() -> Result<Self> {
        let sta = UiaStaThread::new()?;
        Ok(Self { sta })
    }
}

impl AccessibilityWatcher for WindowsUIAWatcher {
    fn get_selected_text(&self) -> Result<Option<TextSelection>> {
        self.sta.get_selected_text()
    }

    fn get_focused_app_name(&self) -> Result<String> {
        self.sta.get_focused_app_name()
    }
}

// SAFETY: WindowsUIAWatcher only exposes &self methods through the
// AccessibilityWatcher trait. All UIA operations are serialized through
// the internal mpsc channel — the STA thread processes one request at a
// time. Concurrent callers block on rx.recv() sequentially. Each watcher
// instance owns its own STA thread, so there is no cross-instance contention.
unsafe impl Send for WindowsUIAWatcher {}
unsafe impl Sync for WindowsUIAWatcher {}

/// Start a background thread that tracks the last non-Tauri foreground window.
/// This allows `get_selected_text` to work even after the Tauri window steals focus.
pub fn start_focus_tracking() {
    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(|| {
        thread::Builder::new()
            .name("focus-tracker".into())
            .spawn(|| unsafe {
                extern "system" fn callback(
                    _h_hook: HWINEVENTHOOK,
                    _event: u32,
                    _hwnd: HWND,
                    _id_object: i32,
                    _id_child: i32,
                    _thread: u32,
                    _time: u32,
                ) {
                    if !_hwnd.is_invalid() && _event == EVENT_SYSTEM_FOREGROUND {
                        let mut fg_pid = 0u32;
                        unsafe { GetWindowThreadProcessId(_hwnd, Some(&mut fg_pid)); }
                        let my_pid = unsafe { GetCurrentProcessId() };
                        if fg_pid != my_pid {
                            LAST_FOREGROUND.store(_hwnd.0 as isize, Ordering::Relaxed);
                        }
                    }
                }

                let hook = SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(callback),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );

                if hook.is_invalid() {
                    log::error!("SetWinEventHook failed, grab button may not work correctly");
                    return;
                }

                log::info!("Focus tracking started");

                // Pump messages so the hook callback fires
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    // no-op, just pumping
                }
            })
            .expect("failed to spawn focus-tracker thread");
    });
}
