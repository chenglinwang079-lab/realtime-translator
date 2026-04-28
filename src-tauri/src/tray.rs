use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{
    AppHandle, Manager, Emitter,
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState},
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

use crate::clipboard::watcher::ClipboardWatcher;

/// 全局存储 toggle_watch 菜单项引用
static TOGGLE_WATCH_ITEM: once_cell::sync::Lazy<Arc<Mutex<Option<MenuItem<tauri::Wry>>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(None)));

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

    // 保存 toggle_watch 菜单项引用
    let toggle_watch_clone = toggle_watch.clone();
    tauri::async_runtime::spawn(async move {
        let mut item = TOGGLE_WATCH_ITEM.lock().await;
        *item = Some(toggle_watch_clone);
    });

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

    // 构建托盘图标
    let icon = app.default_window_icon()
        .ok_or_else(|| tauri::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "应用图标未配置",
        )))?
        .clone();
    let _tray = TrayIconBuilder::with_id("main")
        .icon(icon)
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
                            watcher.start(app_clone.clone()).await;
                        }
                        // 异步完成后更新菜单和发送事件
                        let new_state = !is_running;
                        update_tray_menu(&app_clone, new_state).await;
                        let _ = app_clone.emit("watch-state-changed", new_state);
                        log::info!("[Tray] 监听状态切换为: {}", if new_state { "监听中" } else { "已暂停" });
                    });
                }
                "open_settings" => {
                    // 打开设置面板
                    log::info!("[Tray] 打开设置");
                    if let Some(window) = app.get_webview_window("bubble") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        let _ = window.emit("open-settings", ());
                    }
                }
                "show_window" => {
                    // 显示主窗口
                    log::info!("[Tray] 显示主窗口");
                    if let Some(window) = app.get_webview_window("bubble") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    // 退出应用
                    log::info!("[Tray] 退出应用");
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
                    // 左键单击显示/隐藏窗口（单一职责：显示/隐藏窗口）
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("bubble") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.unminimize();
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
/// 通过全局存储的菜单项引用更新文案
pub async fn update_tray_menu(_app: &AppHandle, is_watching: bool) {
    let text = if is_watching { "暂停监听" } else { "恢复监听" };
    let item = TOGGLE_WATCH_ITEM.lock().await;
    if let Some(menu_item) = item.as_ref() {
        let _ = menu_item.set_text(text);
    }
    log::info!(
        "[Tray] 更新状态: {}",
        if is_watching { "监听中" } else { "已暂停" }
    );
}
