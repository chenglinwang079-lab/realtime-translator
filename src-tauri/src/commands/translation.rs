use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::translation::engine::{detect_language, decide_target_lang, EngineInfo, TranslationEngine, TranslationResult};
use crate::translation::engine_manager::EngineManager;
use crate::translation::memory_cache::{CacheEntry, MemoryCache};

pub struct EngineManagerState(pub Arc<Mutex<EngineManager>>);

const MAX_TRANSLATE_LENGTH: usize = 5000;

/// 校验翻译文本输入，返回 trim 后的文本或错误
fn validate_translate_text(text: &str) -> Result<&str, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("[TRANSLATE_INPUT] 翻译文本为空".to_string());
    }
    let char_count = trimmed.chars().count();
    if char_count > MAX_TRANSLATE_LENGTH {
        return Err(format!(
            "[TRANSLATE_LENGTH] 翻译文本过长（{} 字符，上限 {}）",
            char_count, MAX_TRANSLATE_LENGTH
        ));
    }
    Ok(trimmed)
}

/// 归一化引擎错误：超时等底层错误转为稳定文案，再加 [TRANSLATE] 前缀
fn wrap_translate_error(err: anyhow::Error) -> String {
    let msg = err.to_string();
    // 先判更具体的超时关键词，避免其他错误文案意外含有 timeout 字样
    if msg.contains("timed out") {
        return "[TRANSLATE] 翻译请求超时，请稍后重试".to_string();
    }
    format!("[TRANSLATE] {}", msg)
}

/// 生成缓存 key：text + engine_id + target_lang 的组合
fn cache_key(text: &str, engine_id: &str, target_lang: &str) -> String {
    // length-prefixed 防止 text 含分隔符时碰撞
    format!("{}:{}\x1f{}\x1f{}", text.len(), text, engine_id, target_lang)
}

#[tauri::command]
pub async fn translate(
    text: String,
    engine_manager: State<'_, EngineManagerState>,
    db: State<'_, Database>,
    mem_cache: State<'_, MemoryCache>,
) -> Result<TranslationResult, String> {
    let trimmed = validate_translate_text(&text)?;
    let source_lang = detect_language(trimmed);
    let target_lang = decide_target_lang(source_lang);

    // 从引擎管理器获取默认引擎（短暂持有锁）
    let (default_engine_id, engine) = {
        let manager = engine_manager.0.lock().await;
        let id = manager.default_engine_id().to_string();
        let eng = manager.get_default()
            .ok_or("[TRANSLATE_ENGINE] 没有可用的翻译引擎")?;
        (id, eng)
    };

    let key = cache_key(trimmed, &default_engine_id, target_lang);

    // 第一级：检查 LRU 内存缓存
    if let Some(entry) = mem_cache.get(&key).await {
        log::debug!("[Cache] 内存命中: {}", &key[..8.min(key.len())]);
        return Ok(TranslationResult {
            translated_text: entry.translated,
            source_lang: entry.source_lang,
            target_lang: entry.target_lang,
            engine_id: entry.engine_id,
            latency_ms: 0,
        });
    }

    // 第二级：检查 SQLite 磁盘缓存
    if let Ok(Some(cached)) = db.get_cache(&key).await {
        log::debug!("[Cache] 磁盘命中: {}", &key[..8.min(key.len())]);
        let entry = CacheEntry {
            translated: cached.translated.clone(),
            engine_id: cached.engine_id.clone(),
            source_lang: cached.source_lang.clone(),
            target_lang: cached.target_lang.clone(),
        };
        mem_cache.put(key.clone(), entry).await;
        return Ok(TranslationResult {
            translated_text: cached.translated,
            source_lang: cached.source_lang,
            target_lang: cached.target_lang,
            engine_id: cached.engine_id,
            latency_ms: 0,
        });
    }

    // 缓存未命中，调用引擎（不持有 EngineManager 锁）
    // 先尝试默认引擎，失败时 fallback 到其他可用引擎
    let result = match engine.translate(trimmed).await {
        Ok(r) => r,
        Err(primary_err) => {
            log::warn!("[EngineManager] 主引擎 {} 失败: {}，尝试 fallback", default_engine_id, primary_err);
            // 一次加锁获取所有 fallback 引擎的 Arc
            let fallback_engines: Vec<(String, Arc<dyn TranslationEngine>)> = {
                let manager = engine_manager.0.lock().await;
                manager.list_engines()
                    .into_iter()
                    .filter(|e| e.available && e.id != default_engine_id)
                    .filter_map(|e| {
                        let eng = manager.get_engine(&e.id)?;
                        Some((e.id, eng))
                    })
                    .collect()
            };
            let mut last_err = wrap_translate_error(primary_err);
            for (fb_id, fb_engine) in &fallback_engines {
                log::debug!("[EngineManager] 使用 fallback 引擎: {}", fb_id);
                match fb_engine.translate(trimmed).await {
                    Ok(r) => return Ok(r),
                    Err(e) => {
                        log::warn!("[EngineManager] Fallback {} 也失败: {}", fb_id, e);
                        last_err = wrap_translate_error(e);
                    }
                }
            }
            return Err(last_err);
        }
    };

    // 写入两级缓存（先 DB 后内存，DB 失败时不污染 LRU）
    if let Err(e) = db.set_cache(
        &key,
        &result.translated_text,
        &result.engine_id,
        &result.source_lang,
        &result.target_lang,
    ).await {
        log::warn!("[Cache] 写入磁盘缓存失败: {}", e);
    }

    mem_cache.put(key.clone(), CacheEntry {
        translated: result.translated_text.clone(),
        engine_id: result.engine_id.clone(),
        source_lang: result.source_lang.clone(),
        target_lang: result.target_lang.clone(),
    }).await;

    log::debug!("[Cache] 写入: {}", &key[..8.min(key.len())]);
    Ok(result)
}

#[tauri::command]
pub async fn translate_with_engine(
    text: String,
    engine_id: String,
    engine_manager: State<'_, EngineManagerState>,
    db: State<'_, Database>,
    mem_cache: State<'_, MemoryCache>,
) -> Result<TranslationResult, String> {
    let trimmed = validate_translate_text(&text)?;
    let source_lang = detect_language(trimmed);
    let target_lang = decide_target_lang(source_lang);

    // 获取指定引擎（短暂持有锁）
    let engine = {
        let manager = engine_manager.0.lock().await;
        manager.get_engine(&engine_id)
            .ok_or(format!("[TRANSLATE_ENGINE] 引擎不存在: {}", engine_id))?
    };

    let key = cache_key(trimmed, &engine_id, target_lang);

    // 第一级：检查 LRU 内存缓存
    if let Some(entry) = mem_cache.get(&key).await {
        log::debug!("[Cache] 内存命中: {}", &key[..8.min(key.len())]);
        return Ok(TranslationResult {
            translated_text: entry.translated,
            source_lang: entry.source_lang,
            target_lang: entry.target_lang,
            engine_id: entry.engine_id,
            latency_ms: 0,
        });
    }

    // 第二级：检查 SQLite 磁盘缓存
    if let Ok(Some(cached)) = db.get_cache(&key).await {
        log::debug!("[Cache] 磁盘命中: {}", &key[..8.min(key.len())]);
        let entry = CacheEntry {
            translated: cached.translated.clone(),
            engine_id: cached.engine_id.clone(),
            source_lang: cached.source_lang.clone(),
            target_lang: cached.target_lang.clone(),
        };
        mem_cache.put(key.clone(), entry).await;
        return Ok(TranslationResult {
            translated_text: cached.translated,
            source_lang: cached.source_lang,
            target_lang: cached.target_lang,
            engine_id: cached.engine_id,
            latency_ms: 0,
        });
    }

    // 缓存未命中，调用引擎（不持有 EngineManager 锁）
    let result = engine.translate(trimmed).await.map_err(wrap_translate_error)?;

    // 写入两级缓存（先 DB 后内存，DB 失败时不污染 LRU）
    if let Err(e) = db.set_cache(
        &key,
        &result.translated_text,
        &result.engine_id,
        &result.source_lang,
        &result.target_lang,
    ).await {
        log::warn!("[Cache] 写入磁盘缓存失败: {}", e);
    }

    mem_cache.put(key.clone(), CacheEntry {
        translated: result.translated_text.clone(),
        engine_id: result.engine_id.clone(),
        source_lang: result.source_lang.clone(),
        target_lang: result.target_lang.clone(),
    }).await;

    log::debug!("[Cache] 写入: {}", &key[..8.min(key.len())]);
    Ok(result)
}

/// 所有支持的引擎（即使未注册也显示在设置中供用户配置）
const ALL_ENGINES: &[(&str, &str)] = &[
    ("tencent-tmt", "腾讯云翻译"),
    ("openai-gpt-4o-mini", "OpenAI GPT-4o-mini"),
    ("deepl-free", "DeepL Free"),
];

#[tauri::command]
pub async fn get_engines(
    engine_manager: State<'_, EngineManagerState>,
) -> Result<Vec<EngineInfo>, String> {
    let manager = engine_manager.0.lock().await;
    let registered = manager.list_engines();

    // 合并：已注册的 + 未注册但已知的引擎
    let mut result: Vec<EngineInfo> = registered;
    for &(id, name) in ALL_ENGINES {
        if !result.iter().any(|e| e.id == id) {
            result.push(EngineInfo {
                id: id.to_string(),
                name: name.to_string(),
                available: false,
            });
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn set_default_engine(
    engine_id: String,
    engine_manager: State<'_, EngineManagerState>,
) -> Result<(), String> {
    let mut manager = engine_manager.0.lock().await;
    manager
        .set_default(&engine_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_engine(
    engine_id: String,
    engine_manager: State<'_, EngineManagerState>,
) -> Result<u64, String> {
    let manager = engine_manager.0.lock().await;
    manager
        .health_check(&engine_id)
        .await
        .map_err(|e| e.to_string())
}

/// 清除翻译缓存（同时清除内存和磁盘）
#[tauri::command]
pub async fn clear_cache(db: State<'_, Database>, mem_cache: State<'_, MemoryCache>) -> Result<(), String> {
    mem_cache.clear().await;
    db.clear_cache().await.map_err(|e| e.to_string())?;
    log::debug!("[Cache] 已清空所有缓存");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_translate_text ---

    #[test]
    fn validate_empty_text() {
        assert!(validate_translate_text("").is_err());
        assert!(validate_translate_text("   ").is_err());
        assert!(validate_translate_text("\t\n").is_err());
    }

    #[test]
    fn validate_normal_text() {
        let result = validate_translate_text("  hello world  ");
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn validate_length_limit_chars() {
        // 5000 个中文字符：刚好通过
        let ok_text = "中".repeat(5000);
        assert!(validate_translate_text(&ok_text).is_ok());

        // 5001 个中文字符：超限（15003 字节，但按字符数判断）
        let over_text = "中".repeat(5001);
        let err = validate_translate_text(&over_text).unwrap_err();
        assert!(err.contains("[TRANSLATE_LENGTH]"));
        assert!(err.contains("5001"));
        assert!(err.contains("5000"));
    }

    #[test]
    fn validate_length_exactly_at_limit() {
        let text = "a".repeat(5000);
        assert!(validate_translate_text(&text).is_ok());
    }

    // --- wrap_translate_error ---

    #[test]
    fn wrap_timeout_error() {
        let err = anyhow::anyhow!("request timed out");
        let wrapped = wrap_translate_error(err);
        assert_eq!(wrapped, "[TRANSLATE] 翻译请求超时，请稍后重试");
    }

    #[test]
    fn wrap_normal_error() {
        let err = anyhow::anyhow!("DeepL API 错误 (401): Invalid key");
        let wrapped = wrap_translate_error(err);
        assert!(wrapped.starts_with("[TRANSLATE] DeepL API 错误 (401)"));
    }

    #[test]
    fn wrap_no_double_prefix() {
        let err = anyhow::anyhow!("DeepL API 错误 (401)");
        let wrapped = wrap_translate_error(err);
        assert!(!wrapped.contains("[TRANSLATE] [TRANSLATE]"));
    }
}
