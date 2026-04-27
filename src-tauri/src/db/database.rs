use anyhow::{Context, Result};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// 打开或创建数据库，位于 ~/.realtime-translator/data.db
    pub async fn new() -> Result<Self> {
        let data_dir = dirs::data_dir()
            .context("无法获取用户数据目录")?
            .join("realtime-translator");

        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("data.db");
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&db_url)
            .await
            .context("连接数据库失败")?;

        // 运行迁移
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("数据库迁移失败")?;

        Ok(Self { pool })
    }

    /// 获取连接池引用
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // === Translation History ===

    pub async fn insert_history(
        &self,
        id: &str,
        original: &str,
        translated: &str,
        source_lang: &str,
        target_lang: &str,
        engine_id: &str,
        latency_ms: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO translation_history (id, original, translated, source_lang, target_lang, engine_id, latency_ms) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(original)
        .bind(translated)
        .bind(source_lang)
        .bind(target_lang)
        .bind(engine_id)
        .bind(latency_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_history(&self, limit: i64) -> Result<Vec<HistoryRow>> {
        let rows = sqlx::query_as::<_, HistoryRow>(
            "SELECT id, original, translated, source_lang, target_lang, engine_id, latency_ms, created_at FROM translation_history ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn clear_history(&self) -> Result<()> {
        sqlx::query("DELETE FROM translation_history")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // === Settings ===

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM settings WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(v,)| v))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // === Translation Cache ===

    pub async fn get_cache(&self, cache_key: &str) -> Result<Option<CacheRow>> {
        let row = sqlx::query_as::<_, CacheRow>(
            "SELECT cache_key, translated, engine_id, source_lang, target_lang, created_at FROM translation_cache WHERE cache_key = ?",
        )
        .bind(cache_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn set_cache(
        &self,
        cache_key: &str,
        translated: &str,
        engine_id: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO translation_cache (cache_key, translated, engine_id, source_lang, target_lang) VALUES (?, ?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET translated = excluded.translated, engine_id = excluded.engine_id",
        )
        .bind(cache_key)
        .bind(translated)
        .bind(engine_id)
        .bind(source_lang)
        .bind(target_lang)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_cache(&self) -> Result<()> {
        sqlx::query("DELETE FROM translation_cache")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // === Engine Configs ===

    pub async fn get_engine_config(&self, engine_id: &str) -> Result<Option<EngineConfigRow>> {
        let row = sqlx::query_as::<_, EngineConfigRow>(
            "SELECT engine_id, api_key, base_url, is_default, extra_json FROM engine_configs WHERE engine_id = ?",
        )
        .bind(engine_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_all_engine_configs(&self) -> Result<Vec<EngineConfigRow>> {
        let rows = sqlx::query_as::<_, EngineConfigRow>(
            "SELECT engine_id, api_key, base_url, is_default, extra_json FROM engine_configs",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn set_engine_api_key(&self, engine_id: &str, api_key: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO engine_configs (engine_id, api_key) VALUES (?, ?) ON CONFLICT(engine_id) DO UPDATE SET api_key = excluded.api_key",
        )
        .bind(engine_id)
        .bind(api_key)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_engine_api_key(&self, engine_id: &str) -> Result<()> {
        sqlx::query("UPDATE engine_configs SET api_key = NULL WHERE engine_id = ?")
            .bind(engine_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_engine_extra(&self, engine_id: &str, extra: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO engine_configs (engine_id, extra_json) VALUES (?, ?) ON CONFLICT(engine_id) DO UPDATE SET extra_json = excluded.extra_json",
        )
        .bind(engine_id)
        .bind(extra)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct HistoryRow {
    pub id: String,
    pub original: String,
    pub translated: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine_id: String,
    pub latency_ms: i64,
    pub created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CacheRow {
    pub cache_key: String,
    pub translated: String,
    pub engine_id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EngineConfigRow {
    pub engine_id: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub is_default: i32,
    pub extra_json: Option<String>,
}
