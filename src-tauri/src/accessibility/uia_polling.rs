use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use uiautomation::patterns::UITextPattern;
use uiautomation::UIAutomation;

/// Shared timestamp of the last selection event from the event listener.
/// Updated by `uia_events` when a valid selection-changed event arrives.
/// Checked by the polling fallback to avoid duplicate work.
pub static LAST_SELECTION_EVENT_AT: AtomicU64 = AtomicU64::new(0);

/// Timestamp of the last VS Code diagnostic warning (for rate limiting).
static LAST_VSCODE_WARN_AT: AtomicU64 = AtomicU64::new(0);

/// Maximum characters to read from a text range.
const MAX_SELECTION_CHARS: i32 = 50_000;

/// Polling interval in milliseconds.
const POLL_INTERVAL_MS: u64 = 250;

/// If an event arrived within this window, skip polling.
const EVENT_SUPPRESSION_MS: u64 = 1000;

/// Same text within this window is deduplicated (longer for polling to avoid spam).
const POLLING_DEDUP_MS: u64 = 2000;

/// Candidate window title substrings for browsers/IDEs that don't reliably fire selection events.
const CANDIDATE_TITLE_SUBSTRINGS: &[&str] = &[
    "Chrome",
    "Edge",
    "Visual Studio Code",
    "VS Code",
];

/// Candidate process names — cheap pre-filter before expensive window title traversal.
const CANDIDATE_PROCESS_NAMES: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "code.exe",
    "cursor.exe",
];

/// Minimum text length to emit as a selection event.
const MIN_SELECTION_LEN: usize = 3;

/// Minimum interval between VS Code diagnostic warnings (30 seconds).
const VSCODE_WARN_INTERVAL_MS: u64 = 30_000;

/// Control handle for the UIA polling fallback thread.
pub struct UiaPollingFallback {
    running: AtomicBool,
    stop_tx: std::sync::Mutex<Option<mpsc::Sender<()>>>,
}

impl UiaPollingFallback {
    const fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            stop_tx: std::sync::Mutex::new(None),
        }
    }

    /// Start the polling fallback thread. No-op if already running.
    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            log::info!("UIA Polling: 已在运行，跳过启动");
            return Ok(());
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel();

        // Spawn threads BEFORE setting running flag.
        // This eliminates the window where threads exist but stop_tx is not yet assigned.
        thread::Builder::new()
            .name("uia-polling-fallback".into())
            .spawn(move || {
                if let Err(e) = polling_thread_main(event_tx, stop_rx) {
                    log::error!("UIA Polling 轮询线程错误: {}", e);
                }
            })
            .map_err(|e| format!("Failed to spawn polling thread: {}", e))?;

        thread::Builder::new()
            .name("uia-polling-bridge".into())
            .spawn(move || {
                while let Ok(event) = event_rx.recv() {
                    let _ = app.emit("uia-text-event", &event);
                }
                log::debug!("UIA Polling: Bridge 线程退出");
            })
            .map_err(|e| format!("Failed to spawn polling bridge thread: {}", e))?;

        // Set running and store stop_tx AFTER threads are spawned.
        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(stop_tx);
        } else {
            log::error!("UIA Polling: stop_tx Mutex poisoned");
            return Err("Internal lock error".to_string());
        }
        self.running.store(true, Ordering::SeqCst);

        log::info!("UIA Polling Fallback 已启动");
        Ok(())
    }

    /// Stop the polling fallback thread. No-op if not running.
    pub fn stop(&self) -> Result<(), String> {
        if !self.running.swap(false, Ordering::SeqCst) {
            return Ok(());
        }

        let tx = self.stop_tx.lock().ok().and_then(|mut guard| guard.take());
        if let Some(tx) = tx {
            let _ = tx.send(());
        }

        log::info!("UIA Polling: 停止信号已发送");
        Ok(())
    }

    /// Check if the polling fallback is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Main polling loop.
fn polling_thread_main(
    event_tx: mpsc::Sender<crate::accessibility::uia_events::UiaTextEvent>,
    stop_rx: mpsc::Receiver<()>,
) -> Result<(), String> {
    let automation = UIAutomation::new()
        .map_err(|e| format!("UIAutomation init failed: {}", e))?;

    log::info!("UIA Polling: UIAutomation 已初始化");

    let mut last_text = String::new();
    let mut last_emit = Instant::now() - Duration::from_secs(10);

    loop {
        // Check stop signal.
        match stop_rx.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => {
                log::info!("UIA Polling: 停止信号已收到");
                break;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        // Suppress polling if event channel recently delivered a selection.
        let elapsed_since_event = {
            let ts = LAST_SELECTION_EVENT_AT.load(Ordering::Acquire);
            if ts == 0 {
                u64::MAX // No event ever → don't suppress
            } else {
                now_millis().saturating_sub(ts)
            }
        };

        if elapsed_since_event < EVENT_SUPPRESSION_MS {
            thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            continue;
        }

        // Get the focused element.
        if let Ok(focused) = automation.get_focused_element() {
            // Skip Tauri's own windows.
            if is_tauri_window(&focused) {
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                continue;
            }

            // Cheap pre-filter: check process name before expensive title traversal.
            let app_name = focused
                .get_process_id()
                .map(crate::accessibility::process_name_from_pid)
                .unwrap_or_default();
            if !is_candidate_process(&app_name) {
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                continue;
            }

            // Expensive: walk up to 20 UIA parents to find the window title.
            let window_title = get_top_level_window_title(&focused, &automation);
            if !is_candidate_app(&window_title) {
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                continue;
            }

            // Extract selected text.
            let text = extract_selected_text(&focused);
            if !text.is_empty() && text.trim().len() >= MIN_SELECTION_LEN {
                // Dedup: same text within POLLING_DEDUP_MS.
                let now = Instant::now();
                if text == last_text
                    && now.duration_since(last_emit) < Duration::from_millis(POLLING_DEDUP_MS)
                {
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                    continue;
                }
                last_text = text.clone();
                last_emit = now;

                // 黑名单过滤（进程名精确 + 窗口标题子串，不区分大小写）
                if crate::accessibility::is_blocked(&app_name, &window_title) {
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                    continue;
                }

                log::debug!(
                    "UIA Polling 选择检测: {} chars from {:?}",
                    text.len(),
                    app_name
                );

                let _ = event_tx.send(
                    crate::accessibility::uia_events::UiaTextEvent {
                        text,
                        app_name,
                        window_title,
                        event_type: "selection-changed".to_string(),
                        source: "polling".to_string(),
                    },
                );
            } else if is_vscode_window(&window_title) {
                // Rate-limited hint: VS Code detected but no text captured.
                let now_ts = now_millis();
                let last_warn = LAST_VSCODE_WARN_AT.load(Ordering::Acquire);
                if now_ts.saturating_sub(last_warn) >= VSCODE_WARN_INTERVAL_MS {
                    LAST_VSCODE_WARN_AT.store(now_ts, Ordering::Release);
                    log::debug!(
                        "UIA Polling: VS Code 未获取到文本，可能需启用屏幕阅读器模式 (Shift+Alt+F1)"
                    );
                }
            }
        }

        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    log::info!("UIA Polling: 轮询线程已退出");
    Ok(())
}

/// Check if a process name indicates a candidate application for polling.
/// Cheap pre-filter — avoids expensive window title traversal for non-candidates.
fn is_candidate_process(process_name: &str) -> bool {
    let lower = process_name.to_lowercase();
    CANDIDATE_PROCESS_NAMES
        .iter()
        .any(|candidate| lower == *candidate)
}

/// Check if a window title indicates a candidate application for polling.
fn is_candidate_app(window_title: &str) -> bool {
    CANDIDATE_TITLE_SUBSTRINGS
        .iter()
        .any(|substr| window_title.contains(substr))
}

/// Check if a window title indicates VS Code or a fork (Cursor, etc.).
fn is_vscode_window(title: &str) -> bool {
    let lower = title.to_lowercase();
    lower.contains("visual studio code") || lower.contains("vs code")
}

/// Check if the element belongs to the current (Tauri) process.
fn is_tauri_window(element: &uiautomation::UIElement) -> bool {
    if let Ok(elem_pid) = element.get_process_id() {
        let my_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        return elem_pid == my_pid;
    }
    let class = element.get_classname().unwrap_or_default();
    class.contains("WebView") || class.contains("Tauri")
}

/// Extract selected text via TextPattern only (no ValuePattern fallback).
///
/// In polling mode, we skip ValuePattern because it returns the full element
/// value (not just the selection), which causes false positives in browsers
/// where the page content is exposed as the element's value.
fn extract_selected_text(element: &uiautomation::UIElement) -> String {
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
    String::new()
}

/// Get the window title by traversing up to the top-level Window element.
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

/// Current time in milliseconds since epoch.
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// === Singleton ===

static POLLING: OnceLock<UiaPollingFallback> = OnceLock::new();

pub fn get_polling() -> &'static UiaPollingFallback {
    POLLING.get_or_init(|| UiaPollingFallback::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_candidate_process() {
        assert!(is_candidate_process("chrome.exe"));
        assert!(is_candidate_process("msedge.exe"));
        assert!(is_candidate_process("code.exe"));
        assert!(is_candidate_process("cursor.exe"));
        assert!(!is_candidate_process("notepad.exe"));
        assert!(!is_candidate_process("explorer.exe"));
        // 内部 to_lowercase，大小写不敏感
        assert!(is_candidate_process("Chrome.exe"));
        assert!(is_candidate_process("CHROME.EXE"));
    }

    #[test]
    fn test_is_candidate_app() {
        assert!(is_candidate_app("Chrome - Google"));
        assert!(is_candidate_app("Visual Studio Code"));
        assert!(is_candidate_app("VS Code - My Project"));
        assert!(is_candidate_app("Microsoft Edge"));
        assert!(!is_candidate_app("Notepad"));
        assert!(!is_candidate_app(""));
    }

    #[test]
    fn test_is_vscode_window() {
        assert!(is_vscode_window("Visual Studio Code"));
        assert!(is_vscode_window("VS Code - My Project"));
        assert!(is_vscode_window("visual studio code"));
        assert!(!is_vscode_window("Visual Studio 2022"));
        assert!(!is_vscode_window("Notepad"));
        assert!(!is_vscode_window(""));
    }

    #[test]
    fn test_now_millis() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let ts = now_millis();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(ts >= before && ts <= after);
    }
}
