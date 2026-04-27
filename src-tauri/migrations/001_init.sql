-- RealtimeTranslator v0.1 — Initial Schema

CREATE TABLE IF NOT EXISTS translation_history (
    id          TEXT PRIMARY KEY,
    original    TEXT NOT NULL,
    translated  TEXT NOT NULL,
    source_lang TEXT NOT NULL,
    target_lang TEXT NOT NULL,
    engine_id   TEXT NOT NULL,
    latency_ms  INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_history_created_at ON translation_history(created_at DESC);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS engine_configs (
    engine_id   TEXT PRIMARY KEY,
    api_key     TEXT,
    base_url    TEXT,
    is_default  INTEGER NOT NULL DEFAULT 0,
    extra_json  TEXT
);

CREATE TABLE IF NOT EXISTS translation_cache (
    cache_key   TEXT PRIMARY KEY,
    translated  TEXT NOT NULL,
    engine_id   TEXT NOT NULL,
    source_lang TEXT NOT NULL,
    target_lang TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cache_created_at ON translation_cache(created_at);
