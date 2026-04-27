use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use xcap::Monitor;

/// 显示区域选择器的内部实现（可从命令和快捷键调用）
///
/// 选择器显示在光标所在显示器上（而非固定在主显示器），
/// 支持多显示器环境。
pub fn show_region_selector_impl(app: &AppHandle) -> Result<(), String> {
    // 隐藏气泡窗口
    if let Some(bubble) = app.get_webview_window("bubble") {
        let _ = bubble.hide();
    }

    // 获取光标位置，定位光标所在显示器
    let cursor = crate::window::bubble::get_global_cursor_position();
    let monitor =
        xcap::Monitor::from_point(cursor.x, cursor.y)
            .map_err(|e| format!("未找到光标所在显示器: {}", e))?;

    let mon_x = monitor.x().unwrap_or(0) as i32;
    let mon_y = monitor.y().unwrap_or(0) as i32;
    let mon_w = monitor.width().unwrap_or(1920) as u32;
    let mon_h = monitor.height().unwrap_or(1080) as u32;

    log::info!(
        "光标所在显示器: pos=({}, {}), size={}x{}",
        mon_x, mon_y, mon_w, mon_h
    );

    // 设置 selector 窗口为全屏并显示
    let selector = app
        .get_webview_window("selector")
        .ok_or("Selector 窗口不存在")?;

    selector
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            mon_x, mon_y,
        )))
        .map_err(|e| format!("设置 selector 位置失败: {}", e))?;

    selector
        .set_size(tauri::Size::Physical(tauri::PhysicalSize::new(
            mon_w, mon_h,
        )))
        .map_err(|e| format!("设置 selector 大小失败: {}", e))?;

    selector
        .show()
        .map_err(|e| format!("显示 selector 失败: {}", e))?;

    selector
        .set_always_on_top(true)
        .map_err(|e| format!("设置 always_on_top 失败: {}", e))?;

    selector
        .set_focus()
        .map_err(|e| format!("设置焦点失败: {}", e))?;

    log::info!("区域选择器已显示");
    Ok(())
}

/// 显示区域选择器（Tauri 命令）
#[tauri::command]
pub async fn show_region_selector(app: AppHandle) -> Result<(), String> {
    show_region_selector_impl(&app)
}

/// 前端提交选区（窗口内逻辑坐标），Rust 转换为全局物理坐标
///
/// 转换公式：
///   global_x = window_physical_x + floor(logical_x * scale_factor)
///   global_y = window_physical_y + floor(logical_y * scale_factor)
///   global_w = ceil(logical_w * scale_factor)
///   global_h = ceil(logical_h * scale_factor)
#[tauri::command]
pub async fn submit_region_selection(
    window: WebviewWindow,
    app: AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if width < 1.0 || height < 1.0 {
        return Err("选区太小".to_string());
    }

    let scale = window.scale_factor().unwrap_or(1.0);
    let pos = window
        .outer_position()
        .map_err(|e| format!("获取窗口位置失败: {}", e))?;

    // i64 中间计算防止溢出，clamp 到 i32/u32 范围
    let global_x = (pos.x as i64 + (x * scale).floor() as i64)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let global_y = (pos.y as i64 + (y * scale).floor() as i64)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let global_w = ((width * scale).ceil() as u64).min(u32::MAX as u64) as u32;
    let global_h = ((height * scale).ceil() as u64).min(u32::MAX as u64) as u32;

    log::info!(
        "选区: logical=({}, {} {}x{}), scale={}, global=({}, {} {}x{})",
        x, y, width, height, scale, global_x, global_y, global_w, global_h
    );

    // 广播全局物理坐标
    app.emit(
        "region-selected",
        serde_json::json!({
            "x": global_x,
            "y": global_y,
            "width": global_w,
            "height": global_h,
        }),
    )
    .map_err(|e| format!("emit region-selected 失败: {}", e))?;

    // 隐藏 selector，恢复气泡
    hide_and_restore(&app)?;

    Ok(())
}

/// 取消选择（Esc 触发）
#[tauri::command]
pub async fn cancel_region_selection(app: AppHandle) -> Result<(), String> {
    log::info!("区域选择已取消");
    hide_and_restore(&app)?;
    Ok(())
}

/// 隐藏 selector 窗口，恢复气泡窗口
fn hide_and_restore(app: &AppHandle) -> Result<(), String> {
    if let Some(selector) = app.get_webview_window("selector") {
        if let Err(e) = selector.hide() {
            log::error!("隐藏 selector 失败: {}", e);
        }
    }
    if let Some(bubble) = app.get_webview_window("bubble") {
        if let Err(e) = bubble.show() {
            log::error!("显示 bubble 失败: {}", e);
        }
        let _ = bubble.set_always_on_top(true);
        let _ = bubble.set_focus();
    }
    Ok(())
}
