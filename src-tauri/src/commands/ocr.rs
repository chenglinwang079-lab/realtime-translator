use std::sync::Arc;

use base64::Engine;
use tauri::State;
use tokio::sync::Mutex;

use crate::ocr::engine::OcrEngineInfo;
use crate::ocr::engine_manager::OcrEngineManager;

pub struct OcrEngineManagerState(pub Arc<Mutex<OcrEngineManager>>);

/// 所有已知的 OCR 引擎（即使未注册也显示在设置中供用户配置）
const ALL_OCR_ENGINES: &[(&str, &str)] = &[
    ("google-vision", "Google Cloud Vision"),
    ("baidu-ocr", "百度 OCR"),
];

/// OCR 识别
#[tauri::command]
pub async fn ocr_recognize(
    image_base64: String,
    ocr_mgr: State<'_, OcrEngineManagerState>,
) -> Result<crate::ocr::engine::OcrResult, String> {
    if image_base64.is_empty() {
        return Err("图片数据为空".to_string());
    }

    let image_data = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("base64 解码失败: {}", e))?;

    let manager = ocr_mgr.0.lock().await;
    manager
        .recognize(&image_data, None)
        .await
        .map_err(|e| e.to_string())
}

/// 获取所有 OCR 引擎列表
#[tauri::command]
pub async fn get_ocr_engines(
    ocr_mgr: State<'_, OcrEngineManagerState>,
) -> Result<Vec<OcrEngineInfo>, String> {
    let manager = ocr_mgr.0.lock().await;
    let registered = manager.list_engines();

    // 合并：已注册的 + 未注册但已知的引擎
    let mut result: Vec<OcrEngineInfo> = registered;
    for &(id, name) in ALL_OCR_ENGINES {
        if !result.iter().any(|e| e.id == id) {
            result.push(OcrEngineInfo {
                id: id.to_string(),
                name: name.to_string(),
                available: false,
            });
        }
    }
    Ok(result)
}

/// 测试 OCR 引擎可用性
///
/// 语义：仅校验"是否已配置 API Key"，不保证 key 有效、不保证 API 可达
#[tauri::command]
pub async fn test_ocr_engine(
    engine_id: String,
    ocr_mgr: State<'_, OcrEngineManagerState>,
) -> Result<u64, String> {
    let manager = ocr_mgr.0.lock().await;
    manager
        .health_check(&engine_id)
        .await
        .map_err(|e| e.to_string())
}
