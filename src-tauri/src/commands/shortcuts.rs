use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tokio::sync::Mutex;

/// 当前注册的快捷键
pub struct CurrentShortcut(pub Arc<Mutex<String>>);

/// 注册新的全局快捷键（替换旧的）
#[tauri::command]
pub async fn register_shortcut(
    shortcut: String,
    app: AppHandle,
    current: State<'_, CurrentShortcut>,
) -> Result<(), String> {
    let mut current_shortcut = current.0.lock().await;

    // 先注销旧快捷键
    if !current_shortcut.is_empty() {
        if let Err(e) = app.global_shortcut().unregister(current_shortcut.as_str()) {
            eprintln!("[Shortcuts] 注销旧快捷键失败: {}", e);
        }
    }

    // 注册新快捷键
    let app_handle = app.clone();
    let shortcut_clone = shortcut.clone();
    app.global_shortcut()
        .on_shortcut(
            shortcut.as_str(),
            move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            let text = text.trim().to_string();
                            if !text.is_empty() && text.len() <= 5000 {
                                let _ = app_handle.emit(
                                    "clipboard-changed",
                                    serde_json::json!({
                                        "text": text,
                                        "source": "shortcut"
                                    }),
                                );
                            }
                        }
                    }
                }
            },
        )
        .map_err(|e| format!("注册快捷键失败: {}", e))?;

    *current_shortcut = shortcut_clone.clone();
    println!("[Shortcuts] 快捷键已注册: {}", shortcut_clone);
    Ok(())
}

/// 注销指定快捷键
#[tauri::command]
pub async fn unregister_shortcut(
    shortcut: String,
    app: AppHandle,
    current: State<'_, CurrentShortcut>,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister(shortcut.as_str())
        .map_err(|e| format!("注销快捷键失败: {}", e))?;

    let mut current_shortcut = current.0.lock().await;
    if *current_shortcut == shortcut {
        current_shortcut.clear();
    }

    println!("[Shortcuts] 快捷键已注销: {}", shortcut);
    Ok(())
}
