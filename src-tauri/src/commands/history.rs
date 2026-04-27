use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::Database;

/// 历史记录条目（与前端 TranslationEntry 对应）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub original_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine_id: String,
    pub latency_ms: i64,
    pub timestamp: String,
}

/// 保存翻译历史
#[tauri::command]
pub async fn save_history(
    entry: HistoryEntry,
    db: State<'_, Database>,
) -> Result<(), String> {
    db.insert_history(
        &entry.id,
        &entry.original_text,
        &entry.translated_text,
        &entry.source_lang,
        &entry.target_lang,
        &entry.engine_id,
        entry.latency_ms,
    )
    .await
    .map_err(|e| format!("保存历史失败: {}", e))
}

/// 获取翻译历史（最近 N 条）
#[tauri::command]
pub async fn get_history(
    limit: Option<i64>,
    db: State<'_, Database>,
) -> Result<Vec<HistoryEntry>, String> {
    let rows = db
        .get_history(limit.unwrap_or(200))
        .await
        .map_err(|e| format!("读取历史失败: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|r| HistoryEntry {
            id: r.id,
            original_text: r.original,
            translated_text: r.translated,
            source_lang: r.source_lang,
            target_lang: r.target_lang,
            engine_id: r.engine_id,
            latency_ms: r.latency_ms,
            timestamp: r.created_at,
        })
        .collect())
}

/// 清空翻译历史
#[tauri::command]
pub async fn clear_history(db: State<'_, Database>) -> Result<(), String> {
    db.clear_history()
        .await
        .map_err(|e| format!("清空历史失败: {}", e))
}
