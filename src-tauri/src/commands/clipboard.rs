use tauri::{AppHandle, Emitter, State};

use crate::clipboard::watcher::ClipboardWatcher;

#[tauri::command]
pub async fn start_clipboard_watch(
    app: AppHandle,
    watcher: State<'_, ClipboardWatcher>,
) -> Result<(), String> {
    watcher.start(app).await;
    Ok(())
}

#[tauri::command]
pub async fn stop_clipboard_watch(
    watcher: State<'_, ClipboardWatcher>,
) -> Result<(), String> {
    watcher.stop().await;
    Ok(())
}

/// 切换剪贴板监听状态，返回切换后的状态
#[tauri::command]
pub async fn toggle_clipboard_watch(
    app: AppHandle,
    watcher: State<'_, ClipboardWatcher>,
) -> Result<bool, String> {
    if watcher.is_running() {
        watcher.stop().await;
        let _ = app.emit("watch-state-changed", false);
        Ok(false)
    } else {
        watcher.start(app.clone()).await;
        let _ = app.emit("watch-state-changed", true);
        Ok(true)
    }
}

/// 获取当前监听状态
#[tauri::command]
pub async fn get_watch_state(
    watcher: State<'_, ClipboardWatcher>,
) -> Result<bool, String> {
    Ok(watcher.is_running())
}
