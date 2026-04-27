use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// 气泡窗口状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BubbleState {
    /// 观察态: 点击穿透，不抢焦点
    Preview,
    /// 交互态: 可点击操作
    Interactive,
    /// 已固定到侧边栏
    Pinned,
    /// 已关闭
    Dismissed,
}

/// 鼠标坐标
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
}

/// 获取全局鼠标坐标 (Windows API)
#[cfg(target_os = "windows")]
pub fn get_global_cursor_position() -> CursorPosition {
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    use windows::Win32::Foundation::POINT;

    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut point).ok();
    }
    CursorPosition {
        x: point.x,
        y: point.y,
    }
}

/// 获取全局鼠标坐标 (macOS - placeholder)
#[cfg(target_os = "macos")]
pub fn get_global_cursor_position() -> CursorPosition {
    // TODO: implement with CGEventSource
    CursorPosition { x: 0, y: 0 }
}

/// 获取全局鼠标坐标 (Linux - placeholder)
#[cfg(target_os = "linux")]
pub fn get_global_cursor_position() -> CursorPosition {
    // TODO: implement with X11/Wayland
    CursorPosition { x: 0, y: 0 }
}

/// 计算气泡位置，带边缘避让
pub fn calculate_bubble_position(
    anchor: CursorPosition,
    bubble_size: PhysicalSize<u32>,
    screen_width: i32,
    screen_height: i32,
    offset: i32,
) -> PhysicalPosition<i32> {
    let mut x = anchor.x + offset;
    let mut y = anchor.y + offset;

    let bw = bubble_size.width as i32;
    let bh = bubble_size.height as i32;

    // 右边界避让
    if x + bw > screen_width {
        x = anchor.x - bw - offset;
    }

    // 下边界避让
    if y + bh > screen_height {
        y = anchor.y - bh - offset;
    }

    // 左/上边界兜底
    x = x.max(0);
    y = y.max(0);

    PhysicalPosition::new(x, y)
}

/// 设置气泡窗口状态
/// 注意: 目前不启用点击穿透 (set_ignore_cursor_events)，因为从穿透态
/// 切回需要外部触发器 (全局快捷键等)，留到后续版本实现。
pub fn set_bubble_state(window: &WebviewWindow, state: BubbleState) -> Result<(), String> {
    match state {
        BubbleState::Interactive => {
            let _ = window.set_focus();
        }
        _ => {}
    }
    Ok(())
}

/// 移动气泡到指定位置
pub fn move_bubble(window: &WebviewWindow, position: PhysicalPosition<i32>) -> Result<(), String> {
    window
        .set_position(tauri::Position::Physical(position))
        .map_err(|e| e.to_string())
}

/// 显示气泡窗口
pub fn show_bubble(window: &WebviewWindow) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    Ok(())
}

/// 隐藏气泡窗口
pub fn hide_bubble(window: &WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}

/// Tauri 命令: 获取鼠标位置
#[tauri::command]
pub fn get_cursor_position() -> CursorPosition {
    get_global_cursor_position()
}

/// Tauri 命令: 移动气泡到鼠标位置 (绝对定位，用于初始定位)
#[tauri::command]
pub async fn move_bubble_to_cursor(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let cursor = get_global_cursor_position();
    let window_size = window.outer_size().map_err(|e| e.to_string())?;

    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No monitor found")?;
    let screen_size = monitor.size();
    let screen_width = screen_size.width as i32;
    let screen_height = screen_size.height as i32;

    let position = calculate_bubble_position(cursor, window_size, screen_width, screen_height, 16);

    move_bubble(&window, position)
}

/// Tauri 命令: 按鼠标增量移动气泡 (跟随模式，不跳动)
#[tauri::command]
pub async fn move_bubble_follow(app: AppHandle, dx: i32, dy: i32) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let current = window.outer_position().map_err(|e| e.to_string())?;
    let new_x = current.x + dx;
    let new_y = current.y + dy;

    // 边界兜底
    let new_x = new_x.max(0);
    let new_y = new_y.max(0);

    move_bubble(&window, PhysicalPosition::new(new_x, new_y))
}

/// Tauri 命令: 设置气泡状态
#[tauri::command]
pub async fn set_window_state(app: AppHandle, state: BubbleState) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    set_bubble_state(&window, state)
}

/// Tauri 命令: 显示气泡
#[tauri::command]
pub async fn show_bubble_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    show_bubble(&window)
}

/// Tauri 命令: 隐藏气泡
#[tauri::command]
pub async fn hide_bubble_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    hide_bubble(&window)
}

/// Tauri 命令: 设置窗口大小
#[tauri::command]
pub async fn set_window_size(app: AppHandle, width: u32, height: u32) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let size = PhysicalSize::new(width, height);
    window.set_size(tauri::Size::Physical(size)).map_err(|e| e.to_string())?;

    // 确保窗口在屏幕内
    if let Ok(Some(monitor)) = window.current_monitor() {
        let screen_size = monitor.size();
        let pos = window.outer_position().unwrap_or(PhysicalPosition::new(0, 0));

        let mut x = pos.x;
        let mut y = pos.y;

        if x + width as i32 > screen_size.width as i32 {
            x = (screen_size.width as i32 - width as i32).max(0);
        }
        if y + height as i32 > screen_size.height as i32 {
            y = (screen_size.height as i32 - height as i32).max(0);
        }

        let _ = window.set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)));
    }

    Ok(())
}
