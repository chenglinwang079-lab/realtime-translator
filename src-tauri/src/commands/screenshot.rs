/// 截取主显示器全屏，返回 PNG 字节（前端收到 base64）
#[tauri::command]
pub async fn capture_screen() -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(crate::screenshot::capture_primary_monitor_png)
        .await
        .map_err(|e| format!("截图任务失败: {}", e))?
        .map_err(|e| format!("截图失败: {}", e))
}

/// 截取指定屏幕区域，返回 PNG 字节（前端收到 base64）
#[tauri::command]
pub async fn capture_screen_region(x: u32, y: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        crate::screenshot::capture_region_png(x, y, width, height)
    })
    .await
    .map_err(|e| format!("截图任务失败: {}", e))?
    .map_err(|e| format!("截图失败: {}", e))
}
