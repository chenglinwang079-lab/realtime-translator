use std::sync::Arc;

use anyhow::{bail, Context};
use serde::Serialize;

use super::deepl::DeepLEngine;
use super::engine::{EngineInfo, TranslationEngine, TranslationResult};
use super::openai::OpenAiEngine;
use super::tencent::TencentEngine;

/// 引擎管理器：注册、选择、fallback
pub struct EngineManager {
    engines: Vec<Arc<dyn TranslationEngine>>,
    default_engine_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineManagerStatus {
    pub engines: Vec<EngineInfo>,
    pub default_engine: String,
}

impl EngineManager {
    pub fn new() -> Self {
        Self {
            engines: Vec::new(),
            default_engine_id: String::new(),
        }
    }

    /// 注册一个引擎
    pub fn register(&mut self, engine: Arc<dyn TranslationEngine>) {
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
            bail!("引擎不存在: {}", engine_id);
        }
        self.default_engine_id = engine_id.to_string();
        Ok(())
    }

    /// 获取所有引擎信息
    pub fn list_engines(&self) -> Vec<EngineInfo> {
        self.engines
            .iter()
            .map(|e| EngineInfo {
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
    pub fn get_default(&self) -> Option<Arc<dyn TranslationEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() == self.default_engine_id && e.is_available())
            .cloned()
    }

    /// 获取指定引擎
    pub fn get_engine(&self, engine_id: &str) -> Option<Arc<dyn TranslationEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() == engine_id)
            .cloned()
    }

    /// 获取下一个可用引擎（用于 fallback）
    fn get_fallback(&self, exclude_id: &str) -> Option<Arc<dyn TranslationEngine>> {
        self.engines
            .iter()
            .find(|e| e.engine_id() != exclude_id && e.is_available())
            .cloned()
    }

    /// 翻译：使用指定引擎或默认引擎，失败时尝试 fallback
    pub async fn translate(
        &self,
        text: &str,
        preferred_engine: Option<&str>,
    ) -> anyhow::Result<TranslationResult> {
        // 选择引擎
        let primary = if let Some(id) = preferred_engine {
            self.get_engine(id)
                .filter(|e| e.is_available())
                .or_else(|| self.get_default())
        } else {
            self.get_default()
        };

        let primary = primary.context("没有可用的翻译引擎")?;
        let primary_id = primary.engine_id().to_string();

        // 尝试主引擎
        match primary.translate(text).await {
            Ok(result) => Ok(result),
            Err(primary_err) => {
                eprintln!(
                    "[EngineManager] 主引擎 {} 失败: {}，尝试 fallback",
                    primary_id, primary_err
                );

                // 尝试 fallback
                if let Some(fallback) = self.get_fallback(&primary_id) {
                    let fallback_id = fallback.engine_id().to_string();
                    eprintln!("[EngineManager] 使用 fallback 引擎: {}", fallback_id);
                    match fallback.translate(text).await {
                        Ok(result) => Ok(result),
                        Err(fallback_err) => {
                            bail!(
                                "所有引擎失败。主引擎({}): {} | Fallback({}): {}",
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
            .context(format!("引擎不存在: {}", engine_id))?;
        engine.health_check().await
    }

    /// 获取状态
    pub fn status(&self) -> EngineManagerStatus {
        EngineManagerStatus {
            engines: self.list_engines(),
            default_engine: self.default_engine_id.clone(),
        }
    }

    /// 用新 key 重建/注册指定引擎。
    ///
    /// - 腾讯云需要 secret_id + secret_key，通过 `api_key` 和 `extra` 分别传入
    /// - 其他引擎只用 `api_key`
    pub fn reload_engine(
        &mut self,
        engine_id: &str,
        api_key: &str,
        extra: Option<&str>,
    ) -> anyhow::Result<()> {
        let new_engine: Arc<dyn TranslationEngine> = match engine_id {
            "tencent-tmt" => {
                let secret_key = extra.unwrap_or_default();
                if secret_key.is_empty() {
                    bail!("腾讯云需要 secret_key（通过 extra 参数传入）");
                }
                Arc::new(TencentEngine::new_with_keys(api_key.to_string(), secret_key.to_string())?)
            }
            "openai-gpt-4o-mini" => {
                Arc::new(OpenAiEngine::new_with_key(api_key.to_string())?)
            }
            "deepl-free" => {
                Arc::new(DeepLEngine::new_with_key(api_key.to_string())?)
            }
            _ => bail!("未知引擎: {}", engine_id),
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
