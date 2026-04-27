use std::sync::Arc;

use anyhow::{bail, Context};
use serde::Serialize;

use super::engine::{OcrEngine, OcrEngineInfo};
use super::google_vision::GoogleVisionEngine;

/// OCR 引擎管理器：注册、选择、fallback
pub struct OcrEngineManager {
    engines: Vec<Arc<dyn OcrEngine>>,
    default_engine_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrEngineManagerStatus {
    pub engines: Vec<OcrEngineInfo>,
    pub default_engine: String,
}

impl OcrEngineManager {
    pub fn new() -> Self {
        Self {
            engines: Vec::new(),
            default_engine_id: String::new(),
        }
    }

    /// 注册一个引擎
    pub fn register(&mut self, engine: Arc<dyn OcrEngine>) {
        let id = engine.engine_id().to_string();
        if self.default_engine_id.is_empty() && engine.is_available() {
            self.default_engine_id = id;
        }
        self.engines.push(engine);
    }

    /// 设置默认引擎
    pub fn set_default(&mut self, engine_id: &str) -> anyhow::Result<()> {
        let exists = self.engines.iter().any(|e| e.engine_id() == engine_id);
        if !exists {
            bail!("OCR 引擎不存在: {}", engine_id);
        }
        self.default_engine_id = engine_id.to_string();
        Ok(())
    }

    /// 获取所有引擎信息
    pub fn list_engines(&self) -> Vec<OcrEngineInfo> {
        self.engines
            .iter()
            .map(|e| OcrEngineInfo {
                id: e.engine_id().to_string(),
                name: e.engine_name().to_string(),
                available: e.is_available(),
            })
            .collect()
    }

    /// 获取默认引擎 ID
    pub fn default_engine_id(&self) -> &str {
        &self.default_engine_id
    }

    /// 获取默认引擎
    pub fn get_default(&self) -> Option<Arc<dyn OcrEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() == self.default_engine_id && e.is_available())
            .cloned()
    }

    /// 获取指定引擎
    pub fn get_engine(&self, engine_id: &str) -> Option<Arc<dyn OcrEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() == engine_id)
            .cloned()
    }

    /// 获取下一个可用引擎（用于 fallback）
    fn get_fallback(&self, exclude_id: &str) -> Option<Arc<dyn OcrEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() != exclude_id && e.is_available())
            .cloned()
    }

    /// OCR 识别：使用指定引擎或默认引擎，失败时尝试 fallback
    pub async fn recognize(
        &self,
        image_data: &[u8],
        preferred_engine: Option<&str>,
    ) -> anyhow::Result<super::engine::OcrResult> {
        let primary = if let Some(id) = preferred_engine {
            self.get_engine(id)
                .filter(|e| e.is_available())
                .or_else(|| self.get_default())
        } else {
            self.get_default()
        };

        let primary = primary.context("没有可用的 OCR 引擎")?;
        let primary_id = primary.engine_id().to_string();

        match primary.recognize(image_data).await {
            Ok(result) => Ok(result),
            Err(primary_err) => {
                log::warn!(
                    "OCR 主引擎 {} 失败: {}，尝试 fallback",
                    primary_id,
                    primary_err
                );

                if let Some(fallback) = self.get_fallback(&primary_id) {
                    let fallback_id = fallback.engine_id().to_string();
                    log::info!("使用 fallback OCR 引擎: {}", fallback_id);
                    match fallback.recognize(image_data).await {
                        Ok(result) => Ok(result),
                        Err(fallback_err) => {
                            bail!(
                                "所有 OCR 引擎失败。主引擎({}): {} | Fallback({}): {}",
                                primary_id,
                                primary_err,
                                fallback_id,
                                fallback_err
                            );
                        }
                    }
                } else {
                    Err(primary_err)
                }
            }
        }
    }

    /// 健康检查指定引擎
    pub async fn health_check(&self, engine_id: &str) -> anyhow::Result<u64> {
        let engine = self
            .get_engine(engine_id)
            .context(format!("OCR 引擎不存在: {}", engine_id))?;
        engine.health_check().await
    }

    /// 获取状态
    pub fn status(&self) -> OcrEngineManagerStatus {
        OcrEngineManagerStatus {
            engines: self.list_engines(),
            default_engine: self.default_engine_id.clone(),
        }
    }

    /// 用新 key 重建/注册指定引擎。
    ///
    /// 职责边界：仅负责运行态重载（内存替换），不涉及持久化。
    /// engine_id 使用精确匹配。
    pub fn reload_engine(&mut self, engine_id: &str, api_key: &str) -> anyhow::Result<()> {
        let new_engine: Arc<dyn OcrEngine> = match engine_id {
            "google-vision" => {
                Arc::new(GoogleVisionEngine::new_with_key(api_key.to_string())?)
            }
            _ => bail!("未知 OCR 引擎: {}", engine_id),
        };

        // 替换已有或新增
        if let Some(slot) = self.engines.iter_mut().find(|e| e.engine_id() == engine_id) {
            *slot = new_engine;
        } else {
            self.engines.push(new_engine);
        }

        // 重算默认引擎
        self.recalc_default();

        Ok(())
    }

    /// 移除指定引擎
    pub fn remove_engine(&mut self, engine_id: &str) {
        self.engines.retain(|e| e.engine_id() != engine_id);
        self.recalc_default();
    }

    /// 重算默认引擎（第一个可用的）
    fn recalc_default(&mut self) {
        if self.default_engine_id.is_empty()
            || !self.engines.iter().any(|e| {
                e.engine_id() == self.default_engine_id && e.is_available()
            })
        {
            self.default_engine_id = self
                .engines
                .iter()
                .find(|e| e.is_available())
                .map(|e| e.engine_id().to_string())
                .unwrap_or_default();
        }
    }
}
