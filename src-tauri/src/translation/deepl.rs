use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use reqwest::Client;
use serde::Deserialize;

use super::engine::{decide_target_lang, detect_language, TranslationEngine, TranslationResult};

const DEEPL_API_URL: &str = "https://api-free.deepl.com/v2/translate";

#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
    #[serde(rename = "detected_source_language")]
    detected_source_language: Option<String>,
}

pub struct DeepLEngine {
    api_key: String,
    client: Client,
}

impl DeepLEngine {
    /// 从环境变量创建（fallback）
    pub fn new() -> anyhow::Result<Self> {
        let api_key = std::env::var("DEEPL_API_KEY")
            .unwrap_or_default();
        if api_key.is_empty() {
            anyhow::bail!("DEEPL_API_KEY not configured");
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

    /// 将内部语言代码转换为 DeepL 语言代码
    fn to_deepl_lang(lang: &str, is_target: bool) -> &str {
        match (lang, is_target) {
            ("zh", true) => "ZH",
            ("zh", false) => "ZH",
            ("en", true) => "EN-US",
            ("en", false) => "EN",
            _ => lang,
        }
    }
}

#[async_trait::async_trait]
impl TranslationEngine for DeepLEngine {
    fn engine_id(&self) -> &str {
        "deepl-free"
    }

    fn engine_name(&self) -> &str {
        "DeepL Free"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn translate(&self, text: &str) -> anyhow::Result<TranslationResult> {
        let start = Instant::now();

        let source_lang = detect_language(text);
        let target_lang = decide_target_lang(source_lang);

        let source_deepl = Self::to_deepl_lang(source_lang, false);
        let target_deepl = Self::to_deepl_lang(target_lang, true);

        let params = [
            ("text", text),
            ("source_lang", source_deepl),
            ("target_lang", target_deepl),
        ];

        let response = self
            .client
            .post(DEEPL_API_URL)
            .header(
                "Authorization",
                format!("DeepL-Auth-Key {}", self.api_key),
            )
            .form(&params)
            .send()
            .await
            .context("DeepL API 请求失败")?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .context("读取 DeepL 响应失败")?;

        if !status.is_success() {
            bail!("DeepL API 错误 ({}): {}", status, response_text);
        }

        let deepl_response: DeepLResponse =
            serde_json::from_str(&response_text).context("解析 DeepL 响应失败")?;

        let translated_text = deepl_response
            .translations
            .first()
            .map(|t| t.text.clone())
            .context("DeepL 返回了空翻译结果")?;

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
