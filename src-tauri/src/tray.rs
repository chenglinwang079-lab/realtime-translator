use tauri::{
    AppHandle, Manager, Emitter,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

use crate::clipboard::watcher::ClipboardWatcher;

/// 创建系统托盘
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    // 创建菜单项
    let toggle_watch = MenuItem::with_id(
        app,
        "toggle_watch",
        "暂停监听",
        true,
        None::<&str>,
    )?;

    let open_settings = MenuItem::with_id(
        app,
        "open_settings",
        "设置",
        true,
        None::<&str>,
    )?;

    let show_window = MenuItem::with_id(
        app,
        "show_window",
        "显示主窗口",
        true,
        None::<&str>,
    )?;

    let quit = MenuItem::with_id(
        app,
        "quit",
        "退出",
        true,
        None::<&str>,
    )?;

    let separator = PredefinedMenuItem::separator(app)?;

    // 构建菜单
    let menu = Menu::with_items(
        app,
        &[
            &toggle_watch,
            &separator,
            &show_window,
            &open_settings,
            &separator,
            &quit,
        ],
    )?;

    let _app_handle = app.clone();

    // 构建托盘图标
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("RealtimeTranslator")
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "toggle_watch" => {
                    // 切换监听状态
                    let watcher = app.state::<ClipboardWatcher>().inner().clone();
                    let is_running = watcher.is_running();
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if is_running {
                            watcher.stop().await;
                        } else {
                            watcher.start(app_clone).await;
                        }
                    });
                    let new_state = !is_running;
                    let _ = app.emit("watch-state-changed", new_state);
                    println!("[Tray] 监听状态切换为: {}", if new_state { "监听中" } else { "已暂停" });
                }
                "open_settings" => {
                    // 打开设置面板
                    println!("[Tray] 打开设置");
                    if let Some(window) = app.get_webview_window("bubble") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("open-settings", ());
                    }
                }
                "show_window" => {
                    // 显示主窗口
                    println!("[Tray] 显示主窗口");
                    if let Some(window) = app.get_webview_window("bubble") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    // 退出应用
                    println!("[Tray] 退出应用");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // 左键单击显示/隐藏窗口
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("bubble") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

/// 更新托盘菜单状态
pub fn update_tray_menu(app: &AppHandle, is_watching: bool) {
    // TODO: 更新菜单项文本
    // 这需要更复杂的菜单管理，暂时跳过
    println!(
        "[Tray] 更新状态: {}",
        if is_watching { "监听中" } else { "已暂停" }
    );
}
