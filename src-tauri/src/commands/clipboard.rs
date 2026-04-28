use tauri::{AppHandle, Emitter, State};

use crate::clipboard::watcher::ClipboardWatcher;
use crate::tray::update_tray_menu;

#[tauri::command]
pub async fn start_clipboard_watch(
    app: AppHandle,
    watcher: State<'_, ClipboardWatcher>,
) -> Result<(), String> {
    watcher.start(app.clone()).await;
    // 更新托盘菜单
    update_tray_menu(&app, true).await;
    let _ = app.emit("watch-state-changed", true);
    Ok(())
}

#[tauri::command]
pub async fn stop_clipboard_watch(
    app: AppHandle,
    watcher: State<'_, ClipboardWatcher>,
) -> Result<(), String> {
    watcher.stop().await;
    // 更新托盘菜单
    update_tray_menu(&app, false).await;
    let _ = app.emit("watch-state-changed", false);
    Ok(())
}

/// 切换剪贴板监听状态，返回切换后的状态
#[tauri::command]
pub async fn toggle_clipboard_watch(
    app: AppHandle,
    watcher: State<'_, ClipboardWatcher>,
) -> Result<bool, String> {
    let is_running = watcher.is_running();
    if is_running {
        watcher.stop().await;
    } else {
        watcher.start(app.clone()).await;
    }
    let new_state = !is_running;
    // 更新托盘菜单
    update_tray_menu(&app, new_state).await;
    let _ = app.emit("watch-state-changed", new_state);
    Ok(new_state)
}

/// 获取当前监听状态
#[tauri::command]
pub async fn get_watch_state(
    watcher: State<'_, ClipboardWatcher>,
) -> Result<bool, String> {
    Ok(watcher.is_running())
}
