use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 注册默认快捷键 Ctrl+Shift+T
pub fn register_default_shortcuts(app: &AppHandle) -> anyhow::Result<()> {
    let shortcut = "Ctrl+Shift+T";

    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(
        shortcut,
        move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                // 读取剪贴板并触发翻译
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        let text = text.trim().to_string();
                        if !text.is_empty() && text.len() <= 5000 {
                            let _ = app_handle.emit("clipboard-changed", serde_json::json!({
                                "text": text,
                                "source": "shortcut"
                            }));
                        }
                    }
                }
            }
        },
    )?;

    log::info!("全局快捷键已注册: {}", shortcut);

    // PoC 3: Ctrl+Shift+G — 通过 UIA 抓取其他应用选中文本
    let app_handle2 = app.clone();
    app.global_shortcut().on_shortcut(
        "Ctrl+Shift+G",
        move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                let handle = app_handle2.clone();
                // Spawn to avoid blocking the shortcut callback thread.
                // UIA queries can take 1-4 seconds due to matcher timeouts.
                std::thread::spawn(move || {
                    match crate::accessibility::get_watcher() {
                        Ok(watcher) => {
                            match watcher.get_selected_text() {
                                Ok(Some(selection)) => {
                                    log::info!("快捷键抓取成功: {} chars from {}", selection.text.len(), selection.app_name);
                                    let _ = handle.emit("uia-text-captured", serde_json::json!({
                                        "text": selection.text,
                                        "appName": selection.app_name,
                                        "windowClass": selection.window_class,
                                        "windowTitle": selection.window_title,
                                    }));
                                }
                                Ok(None) => {
                                    log::info!("快捷键: 未检测到选中文本");
                                    let _ = handle.emit("uia-text-captured", serde_json::json!({
                                        "text": null,
                                        "error": "未检测到选中文本",
                                    }));
                                }
                                Err(e) => {
                                    log::error!("快捷键抓取失败: {}", e);
                                    let _ = handle.emit("uia-text-captured", serde_json::json!({
                                        "text": null,
                                        "error": e.to_string(),
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("获取 watcher 失败: {}", e);
                        }
                    }
                });
            }
        },
    )?;

    log::info!("UIA 快捷键已注册: Ctrl+Shift+G");

    // Ctrl+, — 打开设置面板
    let app_handle3 = app.clone();
    app.global_shortcut().on_shortcut(
        "Ctrl+,",
        move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                log::info!("打开设置");
                let _ = app_handle3.emit("open-settings", ());
            }
        },
    )?;

    log::info!("设置快捷键已注册: Ctrl+,");

    // Ctrl+Shift+R — 区域截图选择器
    let app_handle4 = app.clone();
    app.global_shortcut().on_shortcut(
        "Ctrl+Shift+R",
        move |_app, _shortcut, event| {
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                log::info!("打开区域选择器");
                if let Err(e) = crate::commands::selector::show_region_selector_impl(&app_handle4) {
                    log::error!("打开区域选择器失败: {}", e);
                }
            }
        },
    )?;

    log::info!("区域选择器快捷键已注册: Ctrl+Shift+R");
    Ok(())
}

/// 注销所有快捷键
pub fn unregister_all_shortcuts(app: &AppHandle) -> anyhow::Result<()> {
    app.global_shortcut().unregister_all()?;
    log::info!("全局快捷键已注销");
    Ok(())
}
