use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::engine::{AsrEngine, AsrResult};

/// Whisper API 引擎
pub struct WhisperAsrEngine {
    api_key: String,
    base_url: String,
    model: String,
    client: Client,
}

impl WhisperAsrEngine {
    /// 创建新的 Whisper ASR 引擎
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            model: "whisper-1".to_string(),
            client: Client::new(),
        }
    }

    /// 设置自定义 base_url
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    /// 设置自定义模型
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }
}

#[async_trait]
impl AsrEngine for WhisperAsrEngine {
    fn engine_id(&self) -> &str {
        "whisper"
    }

    fn engine_name(&self) -> &str {
        "Whisper API"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn recognize(&self, audio: &[f32], sample_rate: u32) -> Result<AsrResult> {
        let start = Instant::now();

        // 将 f32 PCM 转换为 WAV 格式
        let wav_data = convert_to_wav(audio, sample_rate, 1)?;

        // 创建 multipart form
        let file_part = reqwest::multipart::Part::bytes(wav_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "json");

        // 发送请求
        let url = format!("{}/audio/transcriptions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .context("发送 Whisper API 请求失败")?;

        let latency_ms = start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Whisper API 错误 ({}): {}",
                status,
                body
            ));
        }

        let result: WhisperResponse = response
            .json()
            .await
            .context("解析 Whisper API 响应失败")?;

        Ok(AsrResult {
            text: result.text,
            language: result.language,
            confidence: 1.0, // Whisper API 不返回置信度
            engine_id: self.engine_id().to_string(),
            latency_ms,
        })
    }
}

/// Whisper API 响应
#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
    language: Option<String>,
}

/// 将 f32 PCM 转换为 WAV 格式
fn convert_to_wav(pcm: &[f32], sample_rate: u32, channels: u16) -> Result<Vec<u8>> {
    let bits_per_sample: u16 = 32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let data_size = (pcm.len() * 4) as u32; // f32 = 4 bytes
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&3u16.to_le_bytes()); // format: IEEE float
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    // PCM data (f32)
    for &sample in pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(wav)
}
