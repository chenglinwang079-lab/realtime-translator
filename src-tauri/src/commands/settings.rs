use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Database;
use super::ocr::OcrEngineManagerState;
use super::translation::EngineManagerState;

/// 应用设置（与前端 AppSettings 对应）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub default_source_lang: String,
    pub default_target_lang: String,
    pub default_engine: String,
    pub auto_start: bool,
    pub enable_history: bool,
    pub shortcut: String,
    pub enable_uia_auto_translate: bool,
    pub uia_blacklist: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            default_source_lang: "auto".to_string(),
            default_target_lang: "zh".to_string(),
            default_engine: String::new(),
            auto_start: false,
            enable_history: true,
            shortcut: "Ctrl+Shift+T".to_string(),
            enable_uia_auto_translate: true,
            uia_blacklist: Vec::new(),
        }
    }
}

/// 从数据库读取所有设置，缺失的字段使用默认值（单次查询 + HashMap）
#[tauri::command]
pub async fn get_settings(db: State<'_, Database>) -> Result<AppSettings, String> {
    let defaults = AppSettings::default();

    let rows = db.get_all_settings().await.map_err(|e| e.to_string())?;
    let map: HashMap<String, String> = rows.into_iter().collect();

    let get = |key: &str| -> Option<String> { map.get(key).cloned() };
    let get_bool = |key: &str, default: bool| -> bool {
        get(key).and_then(|v| v.parse::<bool>().ok()).unwrap_or(default)
    };

    let uia_blacklist = get("uiaBlacklist")
        .and_then(|v| match serde_json::from_str::<Vec<String>>(&v) {
            Ok(list) => Some(list),
            Err(e) => {
                log::error!("uiaBlacklist 解析失败: {}", e);
                None
            }
        })
        .unwrap_or_default();

    // 首次加载时同步到共享状态（后续变更由 update_settings 实时同步）
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static BLACKLIST_INIT: AtomicBool = AtomicBool::new(false);
        if !BLACKLIST_INIT.swap(true, Ordering::Relaxed) {
            crate::accessibility::set_uia_blacklist(uia_blacklist.clone());
        }
    }

    Ok(AppSettings {
        theme: get("theme").unwrap_or(defaults.theme),
        default_source_lang: get("defaultSourceLang").unwrap_or(defaults.default_source_lang),
        default_target_lang: get("defaultTargetLang").unwrap_or(defaults.default_target_lang),
        default_engine: get("defaultEngine").unwrap_or(defaults.default_engine),
        auto_start: get_bool("autoStart", defaults.auto_start),
        enable_history: get_bool("enableHistory", defaults.enable_history),
        shortcut: get("shortcut").unwrap_or(defaults.shortcut),
        enable_uia_auto_translate: get_bool("enableUiaAutoTranslate", defaults.enable_uia_auto_translate),
        uia_blacklist,
    })
}

/// 更新部分设置（只更新传入的字段）
#[tauri::command]
pub async fn update_settings(
    settings: serde_json::Value,
    db: State<'_, Database>,
) -> Result<(), String> {
    if let Some(obj) = settings.as_object() {
        for (key, value) in obj {
            let str_value = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Bool(b) => b.to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                    serde_json::to_string(value)
                        .map_err(|e| format!("序列化设置 {} 失败: {}", key, e))?
                }
                _ => continue,
            };
            db.set_setting(key, &str_value)
                .await
                .map_err(|e| format!("保存设置 {} 失败: {}", key, e))?;

            // 黑名单变更时同步到共享状态，即时生效
            #[cfg(target_os = "windows")]
            if key == "uiaBlacklist" {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(&str_value) {
                    crate::accessibility::set_uia_blacklist(list);
                }
            }
        }
    }
    Ok(())
}

/// 保存引擎 API Key 并动态重载引擎
#[tauri::command]
pub async fn save_api_key(
    engine_id: String,
    api_key: String,
    extra: Option<String>,
    db: State<'_, Database>,
    engine_mgr: State<'_, EngineManagerState>,
    ocr_mgr: State<'_, OcrEngineManagerState>,
) -> Result<(), String> {
    db.set_engine_api_key(&engine_id, &api_key)
        .await
        .map_err(|e| format!("保存 API Key 失败: {}", e))?;

    // 腾讯云/百度 OCR 的 secret_key 存到 extra_json
    if let Some(ref extra_val) = extra {
        db.set_engine_extra(&engine_id, extra_val)
            .await
            .map_err(|e| format!("保存 extra 配置失败: {}", e))?;
    }

    // 按 engine_id 前缀分流到对应管理器
    if engine_id.starts_with("google-vision") || engine_id == "baidu-ocr" {
        let mut mgr = ocr_mgr.0.lock().await;
        mgr.reload_engine(&engine_id, &api_key, extra.as_deref())
            .map_err(|e| format!("OCR 引擎重载失败: {}", e))?;
        log::info!("OCR 引擎已重载: {}", engine_id);
    } else {
        let mut mgr = engine_mgr.0.lock().await;
        match mgr.reload_engine(&engine_id, &api_key, extra.as_deref()) {
            Ok(()) => log::info!("引擎已重载: {}", engine_id),
            Err(e) => {
                log::error!("引擎重载失败 {}: {}", engine_id, e);
                // 不返回错误 — key 已保存到 DB，重启后会生效
            }
        }
    }

    Ok(())
}

/// 删除引擎 API Key 并移除引擎
#[tauri::command]
pub async fn delete_api_key(
    engine_id: String,
    db: State<'_, Database>,
    engine_mgr: State<'_, EngineManagerState>,
    ocr_mgr: State<'_, OcrEngineManagerState>,
) -> Result<(), String> {
    db.delete_engine_api_key(&engine_id)
        .await
        .map_err(|e| format!("删除 API Key 失败: {}", e))?;

    // 按 engine_id 前缀分流到对应管理器
    if engine_id.starts_with("google-vision") || engine_id == "baidu-ocr" {
        let mut mgr = ocr_mgr.0.lock().await;
        mgr.remove_engine(&engine_id);
        log::info!("OCR 引擎已移除: {}", engine_id);
    } else {
        let mut mgr = engine_mgr.0.lock().await;
        mgr.remove_engine(&engine_id);
        log::info!("引擎已移除: {}", engine_id);
    }

    Ok(())
}
