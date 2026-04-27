use tauri::command;
use tauri::AppHandle;

use crate::accessibility::{self, TextSelection};

/// Attempt to read the selected text from the currently focused UI element.
///
/// Uses Windows UI Automation (TextPattern / ValuePattern) to detect text
/// in other applications. Returns None if no text is selected or the target
/// app doesn't support accessibility patterns.
#[command]
pub async fn get_selected_text() -> Result<Option<TextSelection>, String> {
    log::debug!("命令: get_selected_text");
    let watcher = accessibility::get_watcher().map_err(|e| {
        log::error!("get_watcher 失败: {}", e);
        e.to_string()
    })?;
    watcher
        .get_selected_text()
        .map_err(|e| {
            log::error!("get_selected_text 失败: {}", e);
            e.to_string()
        })
}

/// Get the name of the currently focused application.
#[command]
pub async fn get_focused_app() -> Result<String, String> {
    log::debug!("命令: get_focused_app");
    let watcher = accessibility::get_watcher().map_err(|e| {
        log::error!("get_watcher 失败: {}", e);
        e.to_string()
    })?;
    watcher
        .get_focused_app_name()
        .map_err(|e| {
            log::error!("get_focused_app_name 失败: {}", e);
            e.to_string()
        })
}

/// Start the UIA event listener (selection-changed auto-translate).
#[command]
pub async fn start_uia_events(app: AppHandle) -> Result<(), String> {
    log::info!("命令: start_uia_events");
    #[cfg(target_os = "windows")]
    {
        accessibility::uia_events::get_listener().start(app)
            .map_err(|e| {
                log::error!("start_uia_events 失败: {}", e);
                e
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("UIA events not supported on this platform".to_string())
    }
}

/// Stop the UIA event listener.
#[command]
pub async fn stop_uia_events() -> Result<(), String> {
    log::info!("命令: stop_uia_events");
    #[cfg(target_os = "windows")]
    {
        accessibility::uia_events::get_listener().stop()
            .map_err(|e| {
                log::error!("stop_uia_events 失败: {}", e);
                e
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("UIA events not supported on this platform".to_string())
    }
}

/// Check if the UIA event listener is running.
#[command]
pub async fn get_uia_events_state() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(accessibility::uia_events::get_listener().is_running())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

/// Start the UIA polling fallback (for apps that don't fire selection events).
#[command]
pub async fn start_uia_polling_fallback(app: AppHandle) -> Result<(), String> {
    log::info!("命令: start_uia_polling_fallback");
    #[cfg(target_os = "windows")]
    {
        accessibility::uia_polling::get_polling().start(app)
            .map_err(|e| {
                log::error!("start_uia_polling_fallback 失败: {}", e);
                e
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        Err("UIA polling not supported on this platform".to_string())
    }
}

/// Stop the UIA polling fallback.
#[command]
pub async fn stop_uia_polling_fallback() -> Result<(), String> {
    log::info!("命令: stop_uia_polling_fallback");
    #[cfg(target_os = "windows")]
    {
        accessibility::uia_polling::get_polling().stop()
            .map_err(|e| {
                log::error!("stop_uia_polling_fallback 失败: {}", e);
                e
            })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("UIA polling not supported on this platform".to_string())
    }
}

/// Check if the UIA polling fallback is running.
#[command]
pub async fn get_uia_polling_state() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(accessibility::uia_polling::get_polling().is_running())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}
