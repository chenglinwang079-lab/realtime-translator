use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use reqwest::Client;
use serde::Deserialize;

use super::engine::{decide_target_lang, detect_language, TranslationEngine, TranslationResult};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const MODEL: &str = "gpt-4o-mini";

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: OpenAiError,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

pub struct OpenAiEngine {
    api_key: String,
    client: Client,
}

impl OpenAiEngine {
    /// 从环境变量创建（fallback）
    pub fn new() -> anyhow::Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .unwrap_or_default();
        if api_key.is_empty() {
            anyhow::bail!("OPENAI_API_KEY not configured");
        }
        Self::new_with_key(api_key)
    }

    /// 从指定的 key 创建
    pub fn new_with_key(api_key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("创建 HTTP client 失败")?;

        Ok(Self { api_key, client })
    }
}

#[async_trait::async_trait]
impl TranslationEngine for OpenAiEngine {
    fn engine_id(&self) -> &str {
        "openai-gpt-4o-mini"
    }

    fn engine_name(&self) -> &str {
        "OpenAI GPT-4o-mini"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn translate(&self, text: &str) -> anyhow::Result<TranslationResult> {
        let start = Instant::now();

        let source_lang = detect_language(text);
        let target_lang = decide_target_lang(source_lang);

        let system_prompt = format!(
            "You are a translator. Translate the following text from {} to {}. Output only the translation, no explanations.",
            source_lang, target_lang
        );

        let body = serde_json::json!({
            "model": MODEL,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": text }
            ],
            "temperature": 0.3
        });

        let response = self
            .client
            .post(OPENAI_API_URL)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .context("OpenAI API 请求失败")?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .context("读取 OpenAI 响应失败")?;

        if !status.is_success() {
            let error_msg = serde_json::from_str::<ErrorResponse>(&response_text)
                .map(|e| e.error.message)
                .unwrap_or_else(|_| response_text);
            bail!("OpenAI API 错误 ({}): {}", status, error_msg);
        }

        let chat_response: ChatResponse =
            serde_json::from_str(&response_text).context("解析 OpenAI 响应失败")?;

        let translated_text = chat_response
            .choices
            .first()
            .map(|c| c.message.content.trim().to_string())
            .unwrap_or_default();

        if translated_text.is_empty() {
            bail!("OpenAI 返回了空翻译结果");
        }

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(TranslationResult {
            translated_text,
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            engine_id: self.engine_id().to_string(),
            latency_ms,
        })
    }
}
