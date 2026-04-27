use std::sync::Arc;
use std::time::Duration;

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
        return Err("[OCR_INPUT] 图片数据为空".to_string());
    }

    let image_data = base64::engine::general_purpose::STANDARD
        .decode(&image_base64)
        .map_err(|e| format!("[OCR_INPUT] base64 解码失败: {}", e))?;

    // 1. 取出引擎 Arc，释放 Mutex
    let primary = {
        let manager = ocr_mgr.0.lock().await;
        manager.get_default_or_preferred(None)
    }; // MutexGuard 在此释放

    let primary = primary.ok_or("[OCR_ENGINE] 没有可用的 OCR 引擎")?;
    let primary_id = primary.engine_id().to_string();

    // 2. 在无锁状态下执行 OCR，包裹超时
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        primary.recognize(&image_data).await
    })
    .await
    .map_err(|_| "[OCR_TIMEOUT] OCR 识别超时".to_string())?
    .map_err(|e| format!("[OCR] {}", e));

    // 3. 主引擎失败 → 尝试 fallback（同样无锁）
    match result {
        Ok(ok) => Ok(ok),
        Err(primary_err) => {
            let fallback = {
                let manager = ocr_mgr.0.lock().await;
                manager.get_fallback_engine(&primary_id)
            };
            if let Some(fb) = fallback {
                let fb_id = fb.engine_id().to_string();
                tokio::time::timeout(Duration::from_secs(30), async {
                    fb.recognize(&image_data).await
                })
                .await
                .map_err(|_| "[OCR_TIMEOUT] Fallback OCR 超时".to_string())?
                // [OCR_FALLBACK] 前缀由前端 friendlyMessage 映射为通用提示；
                // 原始错误详情仅用于调试日志，不直接展示给用户
                .map_err(|e| {
                    format!(
                        "[OCR_FALLBACK] 所有引擎失败。主({}): {} | Fb({}): {}",
                        primary_id, primary_err, fb_id, e
                    )
                })
            } else {
                Err(primary_err)
            }
        }
    }
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
