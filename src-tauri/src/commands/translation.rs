use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::db::Database;
use crate::translation::engine::{detect_language, decide_target_lang, EngineInfo, TranslationResult};
use crate::translation::engine_manager::EngineManager;
use crate::translation::memory_cache::{CacheEntry, MemoryCache};

pub struct EngineManagerState(pub Arc<Mutex<EngineManager>>);

/// 生成缓存 key：text + engine_id + target_lang 的组合
fn cache_key(text: &str, engine_id: &str, target_lang: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    engine_id.hash(&mut hasher);
    target_lang.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[tauri::command]
pub async fn translate(
    text: String,
    engine_manager: State<'_, EngineManagerState>,
    db: State<'_, Database>,
    mem_cache: State<'_, MemoryCache>,
) -> Result<TranslationResult, String> {
    let source_lang = detect_language(&text);
    let target_lang = decide_target_lang(source_lang);

    // 从引擎管理器获取默认引擎（短暂持有锁）
    let (default_engine_id, engine) = {
        let manager = engine_manager.0.lock().await;
        let id = manager.default_engine_id().to_string();
        let eng = manager.get_default()
            .ok_or("没有可用的翻译引擎")?;
        (id, eng)
    };

    let key = cache_key(&text, &default_engine_id, target_lang);

    // 第一级：检查 LRU 内存缓存
    if let Some(entry) = mem_cache.get(&key).await {
        println!("[Cache] 内存命中: {}", &key[..8.min(key.len())]);
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
        println!("[Cache] 磁盘命中: {}", &key[..8.min(key.len())]);
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
    let result = match engine.translate(&text).await {
        Ok(r) => r,
        Err(primary_err) => {
            eprintln!("[EngineManager] 主引擎 {} 失败: {}，尝试 fallback", default_engine_id, primary_err);
            let fallback_ids: Vec<String> = {
                let manager = engine_manager.0.lock().await;
                manager.list_engines()
                    .into_iter()
                    .filter(|e| e.available && e.id != default_engine_id)
                    .map(|e| e.id)
                    .collect()
            };
            let mut last_err = primary_err;
            for fb_id in &fallback_ids {
                if let Some(fb_engine) = {
                    engine_manager.0.lock().await.get_engine(fb_id)
                } {
                    eprintln!("[EngineManager] 使用 fallback 引擎: {}", fb_id);
                    match fb_engine.translate(&text).await {
                        Ok(r) => return Ok(r),
                        Err(e) => {
                            eprintln!("[EngineManager] Fallback {} 也失败: {}", fb_id, e);
                            last_err = e;
                        }
                    }
                }
            }
            return Err(format!("所有引擎失败: {}", last_err));
        }
    };

    // 写入两级缓存
    mem_cache.put(key.clone(), CacheEntry {
        translated: result.translated_text.clone(),
        engine_id: result.engine_id.clone(),
        source_lang: result.source_lang.clone(),
        target_lang: result.target_lang.clone(),
    }).await;

    let _ = db.set_cache(
        &key,
        &result.translated_text,
        &result.engine_id,
        &result.source_lang,
        &result.target_lang,
    ).await;

    println!("[Cache] 写入: {}", &key[..8.min(key.len())]);
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
    let source_lang = detect_language(&text);
    let target_lang = decide_target_lang(source_lang);

    // 获取指定引擎（短暂持有锁）
    let engine = {
        let manager = engine_manager.0.lock().await;
        manager.get_engine(&engine_id)
            .ok_or(format!("引擎不存在: {}", engine_id))?
    };

    let key = cache_key(&text, &engine_id, target_lang);

    // 第一级：检查 LRU 内存缓存
    if let Some(entry) = mem_cache.get(&key).await {
        println!("[Cache] 内存命中: {}", &key[..8.min(key.len())]);
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
        println!("[Cache] 磁盘命中: {}", &key[..8.min(key.len())]);
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
    let result = engine.translate(&text).await.map_err(|e| e.to_string())?;

    // 写入两级缓存
    mem_cache.put(key.clone(), CacheEntry {
        translated: result.translated_text.clone(),
        engine_id: result.engine_id.clone(),
        source_lang: result.source_lang.clone(),
        target_lang: result.target_lang.clone(),
    }).await;

    let _ = db.set_cache(
        &key,
        &result.translated_text,
        &result.engine_id,
        &result.source_lang,
        &result.target_lang,
    ).await;

    println!("[Cache] 写入: {}", &key[..8.min(key.len())]);
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
    println!("[Cache] 已清空所有缓存");
    Ok(())
}
