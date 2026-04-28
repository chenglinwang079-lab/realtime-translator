use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::engine::{decide_target_lang, detect_language, TranslationEngine, TranslationResult};

type HmacSha256 = Hmac<Sha256>;

const API_ENDPOINT: &str = "https://tmt.tencentcloudapi.com";
const SERVICE: &str = "tmt";
const ACTION: &str = "TextTranslate";
const VERSION: &str = "2018-03-21";
const REGION: &str = "ap-guangzhou";
const ALGORITHM: &str = "TC3-HMAC-SHA256";

#[derive(Serialize)]
struct TranslateRequest {
    #[serde(rename = "SourceText")]
    source_text: String,
    #[serde(rename = "Source")]
    source: String,
    #[serde(rename = "Target")]
    target: String,
    #[serde(rename = "ProjectId")]
    project_id: u32,
}

#[derive(Deserialize)]
struct TranslateResponseInner {
    #[serde(rename = "Response")]
    response: TranslateResponseBody,
}

#[derive(Deserialize)]
struct TranslateResponseBody {
    #[serde(rename = "TargetText")]
    target_text: Option<String>,
    #[serde(rename = "Error")]
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(rename = "Code")]
    code: String,
    #[serde(rename = "Message")]
    message: String,
}

pub struct TencentEngine {
    secret_id: String,
    secret_key: String,
    client: Client,
}

impl TencentEngine {
    /// 从环境变量创建（fallback）
    pub fn new() -> anyhow::Result<Self> {
        let secret_id = std::env::var("TENCENT_SECRET_ID")
            .unwrap_or_default();
        let secret_key = std::env::var("TENCENT_SECRET_KEY")
            .unwrap_or_default();
        if secret_id.is_empty() || secret_key.is_empty() {
            anyhow::bail!("Tencent credentials not configured");
        }
        Self::new_with_keys(secret_id, secret_key)
    }

    /// 从指定的 key 创建
    pub fn new_with_keys(secret_id: String, secret_key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("创建 HTTP client 失败")?;

        Ok(Self {
            secret_id,
            secret_key,
            client,
        })
    }

    fn build_authorization(&self, timestamp: i64, payload: &str) -> (String, String) {
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        let hashed_payload = hex_digest(payload);
        let canonical_request = format!(
            "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:tmt.tencentcloudapi.com\n\ncontent-type;host\n{}",
            hashed_payload
        );

        let credential_scope = format!("{}/{}/tc3_request", date, SERVICE);
        let hashed_request = hex_digest(&canonical_request);
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            ALGORITHM, timestamp, credential_scope, hashed_request
        );

        let secret_date =
            hmac_sha256(format!("TC3{}", self.secret_key).as_bytes(), date.as_bytes());
        let secret_service = hmac_sha256(&secret_date, SERVICE.as_bytes());
        let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
        let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes()));

        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders=content-type;host, Signature={}",
            ALGORITHM, self.secret_id, credential_scope, signature
        );

        (authorization, timestamp.to_string())
    }
}

fn hex_digest(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length is valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[async_trait::async_trait]
impl TranslationEngine for TencentEngine {
    fn engine_id(&self) -> &str {
        "tencent-tmt"
    }

    fn engine_name(&self) -> &str {
        "腾讯云翻译"
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn translate(&self, text: &str) -> anyhow::Result<TranslationResult> {
        let start = Instant::now();

        let source_lang = detect_language(text);
        let target_lang = decide_target_lang(source_lang);

        let request_body = TranslateRequest {
            source_text: text.to_string(),
            source: source_lang.to_string(),
            target: target_lang.to_string(),
            project_id: 0,
        };
        let payload = serde_json::to_string(&request_body).context("序列化请求失败")?;

        let timestamp = Utc::now().timestamp();
        let (authorization, ts_str) = self.build_authorization(timestamp, &payload);

        let response = self
            .client
            .post(API_ENDPOINT)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Host", "tmt.tencentcloudapi.com")
            .header("X-TC-Action", ACTION)
            .header("X-TC-Version", VERSION)
            .header("X-TC-Timestamp", &ts_str)
            .header("X-TC-Region", REGION)
            .header("Authorization", &authorization)
            .body(payload)
            .send()
            .await
            .context("腾讯云 API 请求失败")?;

        let status = response.status();
        let response_text = response.text().await.context("读取腾讯云响应失败")?;

        if !status.is_success() {
            bail!("腾讯云 API HTTP 错误 ({}): {}", status, response_text);
        }

        let api_response: TranslateResponseInner =
            serde_json::from_str(&response_text).context("解析腾讯云响应失败")?;

        let body = api_response.response;

        if let Some(err) = body.error {
            bail!("腾讯云 API 错误 ({}): {}", err.code, err.message);
        }

        let translated_text = body
            .target_text
            .filter(|s| !s.is_empty())
            .context("腾讯云返回了空翻译结果")?;

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
