use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod uia_events;

#[cfg(target_os = "windows")]
pub mod uia_polling;

/// 用户配置的应用黑名单（进程名 or 窗口标题关键词）
#[cfg(target_os = "windows")]
static UIA_BLACKLIST: RwLock<Vec<String>> = RwLock::new(Vec::new());

/// 更新黑名单（由 settings 命令调用）
///
/// 自动清理：trim 空白、过滤空条目。
#[cfg(target_os = "windows")]
pub fn set_uia_blacklist(list: Vec<String>) {
    let cleaned: Vec<String> = list
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if let Ok(mut bl) = UIA_BLACKLIST.write() {
        log::debug!("黑名单已更新: {} 项", cleaned.len());
        *bl = cleaned;
    } else {
        log::error!("黑名单更新失败: RwLock poisoned");
    }
}

/// 检查是否命中黑名单
///
/// - 进程名：精确匹配，不区分大小写
/// - 窗口标题：子串匹配，不区分大小写
/// - 任一命中即返回 true
#[cfg(target_os = "windows")]
pub fn is_blocked(app_name: &str, window_title: &str) -> bool {
    let bl = match UIA_BLACKLIST.read() {
        Ok(guard) => guard,
        Err(_) => {
            log::error!("黑名单检查失败: RwLock poisoned");
            return false;
        }
    };
    if bl.is_empty() {
        return false;
    }
    let app_lower = app_name.to_lowercase();
    let title_lower = window_title.to_lowercase();
    bl.iter().any(|entry| {
        let e = entry.to_lowercase();
        app_lower == e || title_lower.contains(&e)
    })
}

/// Get the process executable name from a process ID.
///
/// Returns the filename portion only (e.g. "chrome.exe"), not the full path.
/// Falls back to empty string if the process cannot be queried.
#[cfg(target_os = "windows")]
pub fn process_name_from_pid(pid: u32) -> String {
    use ::windows::Win32::Foundation::CloseHandle;
    use ::windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
    use ::windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) {
            Ok(h) => h,
            Err(e) => {
                log::debug!("OpenProcess 失败 (pid={}): {}", pid, e);
                return String::new();
            }
        };

        let mut buf = [0u16; 260];
        let len = GetModuleFileNameExW(Some(handle), None, &mut buf);
        let _ = CloseHandle(handle);

        if len == 0 {
            return String::new();
        }

        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\')
            .next()
            .unwrap_or("")
            .to_lowercase()
    }
}

/// Represents a text selection detected in another application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSelection {
    /// The selected text content.
    pub text: String,
    /// Name of the source application (e.g., "Notepad", "Chrome").
    pub app_name: String,
    /// Class name of the foreground window.
    pub window_class: String,
    /// Title of the foreground window.
    pub window_title: String,
}

/// Trait for platform-specific accessibility watchers.
///
/// Implementations use OS APIs (UI Automation on Windows, AXUIElement on macOS)
/// to read text selections from other applications.
pub trait AccessibilityWatcher: Send + Sync {
    /// Attempt to read the current text selection from the focused element.
    ///
    /// Returns `None` if no text is selected or the target app doesn't support it.
    fn get_selected_text(&self) -> anyhow::Result<Option<TextSelection>>;

    /// Get the name of the currently focused application.
    fn get_focused_app_name(&self) -> anyhow::Result<String>;
}

/// Start tracking foreground window changes.
/// Call once during app setup — spawns a background thread that keeps
/// `LAST_FOREGROUND` updated with the last non-Tauri window handle.
#[cfg(target_os = "windows")]
pub fn start_focus_tracking() {
    windows::start_focus_tracking();
}

/// Application-level singleton watcher.
///
/// The STA thread and COM objects are created once on first access and reused
/// for the lifetime of the application, avoiding repeated thread spawning and
/// COM initialization overhead.
#[cfg(target_os = "windows")]
static WATCHER: OnceLock<anyhow::Result<windows::WindowsUIAWatcher>> = OnceLock::new();

/// Get the singleton AccessibilityWatcher, initializing it on first call.
#[cfg(target_os = "windows")]
pub fn get_watcher() -> anyhow::Result<&'static (dyn AccessibilityWatcher + 'static)> {
    WATCHER
        .get_or_init(|| {
            log::info!("初始化 UIA Watcher 单例");
            windows::WindowsUIAWatcher::new()
        })
        .as_ref()
        .map(|w| w as &dyn AccessibilityWatcher)
        .map_err(|e| {
            log::error!("UIA Watcher 初始化失败: {}", e);
            anyhow::anyhow!("{}", e)
        })
}

/// Create a platform-specific AccessibilityWatcher (legacy, prefer get_watcher()).
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn create_watcher() -> anyhow::Result<Box<dyn AccessibilityWatcher>> {
    Ok(Box::new(windows::WindowsUIAWatcher::new()?))
}

#[cfg(not(target_os = "windows"))]
pub fn get_watcher() -> anyhow::Result<&'static dyn AccessibilityWatcher> {
    anyhow::bail!("Accessibility watcher not implemented for this platform")
}

#[cfg(not(target_os = "windows"))]
pub fn create_watcher() -> anyhow::Result<Box<dyn AccessibilityWatcher>> {
    anyhow::bail!("Accessibility watcher not implemented for this platform")
}

#[cfg(not(target_os = "windows"))]
pub fn start_focus_tracking() {
    // no-op on non-Windows platforms
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_set_uia_blacklist_cleans_input() {
        set_uia_blacklist(vec![
            "  chrome.exe  ".into(),
            "".into(),
            "  ".into(),
            "notepad.exe".into(),
        ]);
        let bl = UIA_BLACKLIST.read().unwrap();
        assert_eq!(*bl, vec!["chrome.exe", "notepad.exe"]);
        drop(bl);
        set_uia_blacklist(vec![]);
    }

    #[test]
    #[serial]
    fn test_is_blocked_process_case_insensitive() {
        set_uia_blacklist(vec!["chrome.exe".into()]);
        assert!(is_blocked("chrome.exe", "any title"));
        // 进程名不区分大小写（Windows 进程名大小写不一致）
        assert!(is_blocked("Chrome.exe", "any title"));
        assert!(is_blocked("CHROME.EXE", "any title"));
        assert!(!is_blocked("firefox.exe", "any title"));
        set_uia_blacklist(vec![]);
    }

    #[test]
    #[serial]
    fn test_is_blocked_title_substring() {
        set_uia_blacklist(vec!["My Secret App".into()]);
        assert!(is_blocked("", "My Secret App - Window"));
        // 标题匹配不区分大小写
        assert!(is_blocked("", "my secret app"));
        assert!(!is_blocked("", "Other App"));
        set_uia_blacklist(vec![]);
    }

    #[test]
    #[serial]
    fn test_is_blocked_empty_list() {
        set_uia_blacklist(vec![]);
        assert!(!is_blocked("chrome.exe", "any title"));
    }

    #[test]
    #[serial]
    fn test_is_blocked_unicode() {
        set_uia_blacklist(vec!["微信".into()]);
        assert!(is_blocked("", "微信 - 聊天"));
        assert!(!is_blocked("", "WeChat"));
        set_uia_blacklist(vec![]);
    }
}
