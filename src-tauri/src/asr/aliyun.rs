use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

use super::engine::{AsrEngine, AsrResult};

/// 阿里云实时语音识别引擎
pub struct AliyunAsrEngine {
    app_key: String,
    access_key_id: String,
    access_key_secret: String,
    client: Client,
}

impl AliyunAsrEngine {
    /// 创建新的阿里云 ASR 引擎
    pub fn new(app_key: String, access_key_id: String, access_key_secret: String) -> Self {
        Self {
            app_key,
            access_key_id,
            access_key_secret,
            client: Client::new(),
        }
    }

    /// 从数据库配置创建（安全方式）
    pub fn from_config(app_key: String, access_key_id: String, access_key_secret: String) -> Self {
        Self::new(app_key, access_key_id, access_key_secret)
    }

    /// 从环境变量创建（仅用于开发/测试，生产环境请使用 from_config）
    pub fn from_env() -> Result<Self> {
        let app_key = std::env::var("ALIYUN_ASR_APP_KEY")
            .context("未配置 ALIYUN_ASR_APP_KEY 环境变量")?;
        let access_key_id = std::env::var("ALIYUN_ACCESS_KEY_ID")
            .context("未配置 ALIYUN_ACCESS_KEY_ID 环境变量")?;
        let access_key_secret = std::env::var("ALIYUN_ACCESS_KEY_SECRET")
            .context("未配置 ALIYUN_ACCESS_KEY_SECRET 环境变量")?;

        log::warn!("[AliyunASR] 使用环境变量加载 API Key，生产环境请使用数据库存储");

        Ok(Self::new(app_key, access_key_id, access_key_secret))
    }

    /// 生成 32 位唯一 ID
    fn generate_id() -> String {
        Uuid::new_v4().to_string().replace("-", "")
    }

    /// 将 f32 PCM 48000Hz 转换为 16bit PCM 16000Hz mono
    fn convert_audio(pcm_f32: &[f32], sample_rate: u32) -> Vec<u8> {
        // 如果采样率不是 16000，需要重采样
        let pcm_resampled = if sample_rate != 16000 {
            Self::resample(pcm_f32, sample_rate, 16000)
        } else {
            pcm_f32.to_vec()
        };

        // 转换为 16bit PCM
        let mut result = Vec::with_capacity(pcm_resampled.len() * 2);
        for &sample in &pcm_resampled {
            let sample_i16 = (sample * 32767.0).max(-32768.0).min(32767.0) as i16;
            result.extend_from_slice(&sample_i16.to_le_bytes());
        }
        result
    }

    /// 简单的线性重采样
    fn resample(pcm: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (pcm.len() as f64 / ratio) as usize;
        let mut result = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = (i as f64 * ratio) as usize;
            if src_idx < pcm.len() {
                result.push(pcm[src_idx]);
            }
        }

        result
    }
}

#[async_trait]
impl AsrEngine for AliyunAsrEngine {
    fn engine_id(&self) -> &str {
        "aliyun-asr"
    }

    fn engine_name(&self) -> &str {
        "阿里云实时语音识别"
    }

    fn is_available(&self) -> bool {
        !self.app_key.is_empty() && !self.access_key_id.is_empty() && !self.access_key_secret.is_empty()
    }

    async fn recognize(&self, audio: &[f32], sample_rate: u32) -> Result<AsrResult> {
        let start = Instant::now();

        // 转换音频格式：f32 -> 16bit PCM, 重采样到 16000Hz mono
        let pcm_16bit = Self::convert_audio(audio, sample_rate);

        // 生成 task_id 和 message_id
        let task_id = Self::generate_id();
        let message_id = Self::generate_id();

        // 创建 multipart form
        let file_part = reqwest::multipart::Part::bytes(pcm_16bit)
            .file_name("audio.pcm")
            .mime_str("audio/pcm")?;

        let form = reqwest::multipart::Form::new()
            .part("file", file_part)
            .text("appkey", self.app_key.clone())
            .text("task_id", task_id)
            .text("message_id", message_id)
            .text("format", "pcm")
            .text("sample_rate", "16000")
            .text("enable_punctuation_prediction", "true")
            .text("enable_inverse_text_normalization", "true");

        // 发送请求到阿里云 ASR API
        let url = "https://nls-gateway.cn-shanghai.aliyuncs.com/stream/v1/asr";

        let response = self
            .client
            .post(url)
            .header("X-NLS-Token", &self.access_key_id)
            .multipart(form)
            .send()
            .await
            .context("发送阿里云 ASR 请求失败")?;

        let latency_ms = start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "阿里云 ASR 错误 ({}): {}",
                status,
                body
            ));
        }

        let result: AliyunAsrResponse = response
            .json()
            .await
            .context("解析阿里云 ASR 响应失败")?;

        Ok(AsrResult {
            text: result.result.unwrap_or_default(),
            language: Some("zh".to_string()),
            confidence: result.confidence.unwrap_or(0.9),
            engine_id: self.engine_id().to_string(),
            latency_ms,
        })
    }
}

/// 阿里云 ASR 响应
#[derive(Debug, Deserialize)]
struct AliyunAsrResponse {
    result: Option<String>,
    confidence: Option<f32>,
}
