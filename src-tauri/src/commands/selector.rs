use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use xcap::Monitor;

/// 显示区域选择器的内部实现（可从命令和快捷键调用）
pub fn show_region_selector_impl(app: &AppHandle) -> Result<(), String> {
    // 隐藏气泡窗口
    if let Some(bubble) = app.get_webview_window("bubble") {
        let _ = bubble.hide();
    }

    // 获取主显示器物理位置和尺寸
    let monitors = Monitor::all().map_err(|e| format!("获取显示器列表失败: {}", e))?;
    let primary = monitors
        .into_iter()
        .filter_map(|m| match m.is_primary() {
            Ok(true) => Some(m),
            Ok(false) => None,
            Err(e) => {
                log::warn!("is_primary() 失败: {}", e);
                None
            }
        })
        .next()
        .ok_or("未找到主显示器")?;

    let mon_x = primary.x().map_err(|e| format!("获取显示器 x 失败: {}", e))? as i32;
    let mon_y = primary.y().map_err(|e| format!("获取显示器 y 失败: {}", e))? as i32;
    let mon_w = primary.width().map_err(|e| format!("获取显示器宽度失败: {}", e))? as u32;
    let mon_h = primary.height().map_err(|e| format!("获取显示器高度失败: {}", e))? as u32;

    log::info!(
        "主显示器: pos=({}, {}), size={}x{}",
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

    // 起点 floor / 宽高 ceil，避免 DPI 缩放下边缘偏差
    let global_x = pos.x + (x * scale).floor() as i32;
    let global_y = pos.y + (y * scale).floor() as i32;
    let global_w = (width * scale).ceil() as u32;
    let global_h = (height * scale).ceil() as u32;

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
