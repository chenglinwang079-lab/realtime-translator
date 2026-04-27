use serde::{Deserialize, Serialize};

/// 翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationResult {
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub engine_id: String,
    pub latency_ms: u64,
}

/// 引擎信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
}

/// 翻译引擎 trait
#[async_trait::async_trait]
pub trait TranslationEngine: Send + Sync {
    /// 引擎唯一标识
    fn engine_id(&self) -> &str;

    /// 引擎显示名称
    fn engine_name(&self) -> &str;

    /// 引擎是否可用（如 API Key 是否已配置）
    fn is_available(&self) -> bool;

    /// 翻译文本，自动检测语言方向
    async fn translate(&self, text: &str) -> anyhow::Result<TranslationResult>;

    /// 健康检查（发送测试请求验证连通性）
    async fn health_check(&self) -> anyhow::Result<u64> {
        // 默认实现：尝试翻译一个简单文本
        let result = self.translate("hello").await?;
        Ok(result.latency_ms)
    }
}

/// 语言检测（本地启发式逻辑）
pub fn detect_language(text: &str) -> &'static str {
    let has_chinese = text
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if has_chinese {
        "zh"
    } else {
        "en"
    }
}

/// 决定翻译方向
pub fn decide_target_lang(source_lang: &str) -> &'static str {
    if source_lang == "zh" {
        "en"
    } else {
        "zh"
    }
}
