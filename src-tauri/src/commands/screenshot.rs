use std::time::Duration;

use base64::Engine;

/// 截取主显示器全屏，返回 base64 编码的 PNG
#[tauri::command]
pub async fn capture_screen() -> Result<String, String> {
    let png = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::task::spawn_blocking(crate::screenshot::capture_primary_monitor_png)
            .await
            .map_err(|e| format!("[SCREENSHOT_TASK] 任务失败: {}", e))?
            .map_err(|e| format!("[SCREENSHOT] {}", e))
    })
    .await
    .map_err(|_| "[SCREENSHOT_TIMEOUT] 截图超时".to_string())??;

    Ok(base64::engine::general_purpose::STANDARD.encode(&png))
}

/// 截取指定屏幕区域，返回 base64 编码的 PNG
#[tauri::command]
pub async fn capture_screen_region(x: u32, y: u32, width: u32, height: u32) -> Result<String, String> {
    let png = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::task::spawn_blocking(move || {
            crate::screenshot::capture_region_png(x, y, width, height)
        })
        .await
        .map_err(|e| format!("[SCREENSHOT_TASK] 任务失败: {}", e))?
        .map_err(|e| format!("[SCREENSHOT] {}", e))
    })
    .await
    .map_err(|_| "[SCREENSHOT_TIMEOUT] 截图超时".to_string())??;

    Ok(base64::engine::general_purpose::STANDARD.encode(&png))
}
