use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// 实时语音翻译结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveTranslationPayload {
    /// 原文
    pub transcript_text: String,
    /// 译文
    pub translated_text: String,
    /// 源语言
    pub source_language: Option<String>,
    /// 目标语言
    pub target_language: Option<String>,
    /// 是否为最终结果
    pub is_final: bool,
    /// chunk ID
    pub chunk_id: u32,
    /// 时间戳（毫秒）
    pub timestamp_ms: u64,
    /// 音频时长（毫秒）
    pub duration_ms: Option<u64>,
}

/// 实时语音翻译引擎 trait
#[async_trait]
pub trait SpeechTranslationEngine: Send + Sync {
    /// 引擎 ID
    fn engine_id(&self) -> &str;

    /// 是否可用（API Key 是否配置）
    fn is_available(&self) -> bool;

    /// 设置结果接收通道
    fn set_result_channel(&self, tx: mpsc::Sender<LiveTranslationPayload>);

    /// 开始会话
    async fn start_session(&self) -> anyhow::Result<()>;

    /// 发送音频 chunk
    async fn send_audio_chunk(&self, pcm: &[f32], sample_rate: u32) -> anyhow::Result<()>;

    /// 停止会话
    async fn stop_session(&self) -> anyhow::Result<()>;
}
