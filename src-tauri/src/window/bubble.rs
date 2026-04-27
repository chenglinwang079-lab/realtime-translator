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

/// 计算气泡位置，带边缘避让（多显示器安全）
///
/// `mon_x/mon_y/mon_w/mon_h` 为光标所在显示器的全局物理坐标和尺寸。
pub fn calculate_bubble_position(
    anchor: CursorPosition,
    bubble_size: PhysicalSize<u32>,
    mon_x: i32,
    mon_y: i32,
    mon_w: i32,
    mon_h: i32,
    offset: i32,
) -> PhysicalPosition<i32> {
    let mut x = anchor.x + offset;
    let mut y = anchor.y + offset;

    let bw = bubble_size.width as i32;
    let bh = bubble_size.height as i32;

    // 右边界避让
    if x + bw > mon_x + mon_w {
        x = anchor.x - bw - offset;
    }

    // 下边界避让
    if y + bh > mon_y + mon_h {
        y = anchor.y - bh - offset;
    }

    // 左/上边界兜底（相对于显示器原点，而非 (0,0)）
    x = x.max(mon_x);
    y = y.max(mon_y);

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
///
/// 使用光标所在显示器（而非窗口所在显示器）进行边界计算，
/// 确保气泡在多显示器环境下正确定位。
#[tauri::command]
pub async fn move_bubble_to_cursor(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let cursor = get_global_cursor_position();
    let window_size = window.outer_size().map_err(|e| e.to_string())?;

    // 用光标坐标找到光标所在显示器（而非窗口所在显示器）
    let monitor = xcap::Monitor::from_point(cursor.x, cursor.y)
        .map_err(|e| format!("未找到光标所在显示器: {}", e))?;

    let mon_x = monitor.x().unwrap_or(0) as i32;
    let mon_y = monitor.y().unwrap_or(0) as i32;
    let mon_w = monitor.width().unwrap_or(1920) as i32;
    let mon_h = monitor.height().unwrap_or(1080) as i32;

    let position = calculate_bubble_position(
        cursor, window_size, mon_x, mon_y, mon_w, mon_h, 16,
    );

    move_bubble(&window, position)
}

/// Tauri 命令: 按鼠标增量移动气泡 (跟随模式，不跳动)
///
/// 不限制坐标范围，允许气泡在多显示器间自由移动。
#[tauri::command]
pub async fn move_bubble_follow(app: AppHandle, dx: i32, dy: i32) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let current = window.outer_position().map_err(|e| e.to_string())?;
    let new_x = current.x + dx;
    let new_y = current.y + dy;

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
///
/// 调整大小后确保窗口在当前显示器范围内（多显示器安全）。
#[tauri::command]
pub async fn set_window_size(app: AppHandle, width: u32, height: u32) -> Result<(), String> {
    let window = app
        .get_webview_window("bubble")
        .ok_or("Bubble window not found")?;

    let size = PhysicalSize::new(width, height);
    window.set_size(tauri::Size::Physical(size)).map_err(|e| e.to_string())?;

    // 确保窗口在当前显示器内
    if let Ok(Some(monitor)) = window.current_monitor() {
        let mon_pos = monitor.position();
        let mon_size = monitor.size();
        let pos = window.outer_position().unwrap_or(PhysicalPosition::new(0, 0));

        let mut x = pos.x;
        let mut y = pos.y;

        // 右/下边界
        if x + width as i32 > mon_pos.x + mon_size.width as i32 {
            x = mon_pos.x + mon_size.width as i32 - width as i32;
        }
        if y + height as i32 > mon_pos.y + mon_size.height as i32 {
            y = mon_pos.y + mon_size.height as i32 - height as i32;
        }

        // 左/上边界（相对于显示器原点）
        x = x.max(mon_pos.x);
        y = y.max(mon_pos.y);

        let _ = window.set_position(tauri::Position::Physical(PhysicalPosition::new(x, y)));
    }

    Ok(())
}
