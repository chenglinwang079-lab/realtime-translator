use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 音频格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Pcm,
}

/// ASR 识别结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrResult {
    /// 识别出的文本
    pub text: String,
    /// 检测到的语言
    pub language: Option<String>,
    /// 置信度 (0.0 - 1.0)
    pub confidence: f32,
    /// 引擎 ID
    pub engine_id: String,
    /// 延迟（毫秒）
    pub latency_ms: u64,
}

/// ASR 引擎 trait
#[async_trait]
pub trait AsrEngine: Send + Sync {
    /// 引擎 ID
    fn engine_id(&self) -> &str;

    /// 引擎名称
    fn engine_name(&self) -> &str;

    /// 是否可用（API Key 是否配置）
    fn is_available(&self) -> bool;

    /// 识别音频
    async fn recognize(&self, audio: &[f32], sample_rate: u32) -> anyhow::Result<AsrResult>;

    /// 健康检查
    async fn health_check(&self) -> anyhow::Result<u64> {
        // 默认实现：检查是否可用
        if self.is_available() {
            Ok(0)
        } else {
            Err(anyhow::anyhow!("引擎不可用"))
        }
    }
}
