use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

use super::engine::{OcrEngine, OcrLevel, OcrResult, OcrTextBlock};
use super::group_words_to_lines;
use super::truncate_error_body;

const TOKEN_URL: &str = "https://aip.baidubce.com/oauth/2.0/token";
const OCR_URL: &str = "https://aip.baidubce.com/rest/2.0/ocr/v1/general_basic";

// --- 私有响应类型 ---

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct OcrResponse {
    words_result: Option<Vec<WordsResultItem>>,
    #[allow(dead_code)]
    words_result_num: Option<i64>,
    error_code: Option<i64>,
    error_msg: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
struct WordsResultItem {
    words: String,
    location: Option<Location>,
}

#[derive(Deserialize)]
struct Location {
    top: i64,
    left: i64,
    width: i64,
    height: i64,
}

// --- CachedToken ---

struct CachedToken {
    token: String,
    expires_at: Instant,
}

// --- BaiduOcrEngine ---

pub struct BaiduOcrEngine {
    api_key: String,
    secret_key: String,
    client: Client,
    token: Mutex<Option<CachedToken>>,
}

impl BaiduOcrEngine {
    /// 从环境变量创建（fallback）
    pub fn new() -> anyhow::Result<Self> {
        let api_key = std::env::var("BAIDU_OCR_API_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        let secret_key = std::env::var("BAIDU_OCR_SECRET_KEY")
            .unwrap_or_default()
            .trim()
            .to_string();
        if api_key.is_empty() || secret_key.is_empty() {
            bail!("BAIDU_OCR_API_KEY 或 BAIDU_OCR_SECRET_KEY 未配置");
        }
        Self::new_with_credentials(api_key, secret_key)
    }

    /// 从指定凭据创建
    pub fn new_with_credentials(api_key: String, secret_key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .use_rustls_tls()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("创建 HTTP client 失败")?;
        Ok(Self {
            api_key,
            secret_key,
            client,
            token: Mutex::new(None),
        })
    }

    /// 获取/刷新 OAuth token
    ///
    /// 锁粒度：先读锁检查缓存 → 释放锁 → HTTP 请求 → 再写锁回写。
    /// 允许并发下两个请求同时刷新（thundering herd），首版可接受。
    async fn get_token(&self) -> anyhow::Result<String> {
        // 1. 读锁检查缓存
        {
            let guard = self.token.lock().await;
            if let Some(ref cached) = *guard {
                if cached.expires_at > Instant::now() {
                    return Ok(cached.token.clone());
                }
            }
        } // 读锁释放

        // 2. 无锁状态下发 HTTP 请求
        let url = format!(
            "{}?grant_type=client_credentials&client_id={}&client_secret={}",
            TOKEN_URL, self.api_key, self.secret_key
        );

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .context("百度 OCR Token 请求失败")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("读取百度 Token 响应失败")?;

        if !status.is_success() {
            // 不传入原始 body，避免百度 API 回显 client_id/client_secret 到错误信息
            bail!("[OCR] 百度 OCR Token 获取失败 (HTTP {})", status);
        }

        let token_resp: TokenResponse =
            serde_json::from_str(&body).context("解析百度 Token 响应失败")?;

        if let Some(err) = token_resp.error {
            let desc = token_resp.error_description.unwrap_or_default();
            log::warn!("百度 OCR Token 错误: {} - {}", err, desc);
            bail!("[OCR] 百度 OCR Token 获取失败: {}", truncate_error_body(&desc, 200));
        }

        let access_token = token_resp
            .access_token
            .ok_or_else(|| anyhow::anyhow!("[OCR] 百度 OCR Token 响应缺少 access_token"))?;

        let expires_in = token_resp.expires_in.unwrap_or(3600); // 保守默认 1h，百度实际返回 30 天
        let expires_at = Instant::now() + Duration::from_secs(expires_in.saturating_sub(3600));

        // 3. 写锁回写
        let mut guard = self.token.lock().await;
        *guard = Some(CachedToken {
            token: access_token.clone(),
            expires_at,
        });

        Ok(access_token)
    }

    /// 将百度 location 转换为 [x, y, width, height]
    fn convert_bbox(loc: &Location) -> [f64; 4] {
        [
            loc.left as f64,
            loc.top as f64,
            loc.width as f64,
            loc.height as f64,
        ]
    }

    /// 由矩形 bbox 推导四角 polygon
    fn bbox_to_polygon(bbox: [f64; 4]) -> Vec<[f64; 2]> {
        let [x, y, w, h] = bbox;
        vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
    }
}

#[async_trait::async_trait]
impl OcrEngine for BaiduOcrEngine {
    fn engine_id(&self) -> &str {
        "baidu-ocr"
    }

    fn engine_name(&self) -> &str {
        "百度 OCR"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty() && !self.secret_key.is_empty()
    }

    async fn recognize(&self, image_data: &[u8]) -> anyhow::Result<OcrResult> {
        let start = Instant::now();

        // 1. 获取 token
        let token = self.get_token().await?;

        // 2. 构造请求
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_data);
        let url = format!("{}?access_token={}", OCR_URL, token);

        let response = self
            .client
            .post(&url)
            .form(&[("image", &image_base64)])
            .send()
            .await
            .context("百度 OCR API 请求失败")?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .context("读取百度 OCR 响应失败")?;

        let latency_ms = start.elapsed().as_millis() as u64;

        if !status.is_success() {
            bail!(
                "[OCR] 百度 OCR 请求失败 (HTTP {}): {}",
                status,
                truncate_error_body(&response_text, 200)
            );
        }

        let ocr_resp: OcrResponse =
            serde_json::from_str(&response_text).context("解析百度 OCR 响应失败")?;

        // 3. 检查业务错误
        if let Some(code) = ocr_resp.error_code {
            let msg = ocr_resp.error_msg.unwrap_or_default();
            log::warn!("百度 OCR API 错误 code={}, msg={}", code, msg);
            bail!("[OCR] 百度 OCR 错误 (code={})", code);
        }

        // 4. 解析结果
        let items = ocr_resp.words_result.unwrap_or_default();

        if items.is_empty() {
            return Ok(OcrResult {
                blocks: vec![],
                full_text: String::new(),
                language: None,
                engine_id: self.engine_id().to_string(),
                latency_ms,
            });
        }

        let words: Vec<OcrTextBlock> = items
            .iter()
            .map(|item| {
                let bbox = item.location.as_ref().map(Self::convert_bbox);
                let polygon = bbox.map(Self::bbox_to_polygon);
                let font_size = bbox.map(|b| b[3]); // height = estimated font size

                OcrTextBlock {
                    text: item.words.clone(),
                    bbox,
                    polygon,
                    confidence: None,
                    font_size,
                    level: OcrLevel::Word,
                }
            })
            .collect();

        // 行分组
        let lines = group_words_to_lines(&words);

        // 无 bbox 的 word 保留
        let no_bbox_words: Vec<&OcrTextBlock> =
            words.iter().filter(|w| w.bbox.is_none()).collect();

        // full_text：line texts 用 \n 连接 + 无 bbox word texts 追加
        let mut full_text_parts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        for w in &no_bbox_words {
            full_text_parts.push(w.text.clone());
        }
        let full_text = full_text_parts.join("\n");

        // 合并 blocks：lines 在前，words 在后（与 Google Vision 一致）
        let mut blocks = lines;
        blocks.extend(words);

        Ok(OcrResult {
            blocks,
            full_text,
            language: None, // general_basic 不返回语言
            engine_id: self.engine_id().to_string(),
            latency_ms,
        })
    }

    /// 覆盖默认实现：仅检查凭据是否已配置，不发 API 请求
    async fn health_check(&self) -> anyhow::Result<u64> {
        if self.api_key.is_empty() || self.secret_key.is_empty() {
            bail!("百度 OCR API Key 或 Secret Key 未配置");
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ocr_response() {
        let json = r#"{
            "words_result": [
                {
                    "words": "你好世界",
                    "location": { "top": 10, "left": 20, "width": 100, "height": 30 }
                },
                {
                    "words": "Hello World",
                    "location": { "top": 50, "left": 20, "width": 120, "height": 20 }
                }
            ],
            "words_result_num": 2
        }"#;

        let resp: OcrResponse = serde_json::from_str(json).unwrap();
        let items = resp.words_result.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].words, "你好世界");
        assert_eq!(items[1].words, "Hello World");

        let loc = items[0].location.as_ref().unwrap();
        assert_eq!(loc.top, 10);
        assert_eq!(loc.left, 20);
        assert_eq!(loc.width, 100);
        assert_eq!(loc.height, 30);
    }

    #[test]
    fn parse_empty_response() {
        let json = r#"{
            "words_result": [],
            "words_result_num": 0
        }"#;

        let resp: OcrResponse = serde_json::from_str(json).unwrap();
        assert!(resp.words_result.unwrap().is_empty());
        assert_eq!(resp.words_result_num, Some(0));
    }

    #[test]
    fn parse_error_response() {
        let json = r#"{
            "error_code": 110,
            "error_msg": "Access token invalid or no longer valid"
        }"#;

        let resp: OcrResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.error_code, Some(110));
        assert!(resp.error_msg.is_some());
        assert!(resp.words_result.is_none());
    }

    #[test]
    fn convert_bbox_works() {
        let loc = Location {
            top: 10,
            left: 20,
            width: 100,
            height: 30,
        };
        let bbox = BaiduOcrEngine::convert_bbox(&loc);
        assert_eq!(bbox, [20.0, 10.0, 100.0, 30.0]);
    }

    #[test]
    fn bbox_to_polygon_works() {
        let poly = BaiduOcrEngine::bbox_to_polygon([20.0, 10.0, 100.0, 30.0]);
        assert_eq!(
            poly,
            vec![
                [20.0, 10.0],
                [120.0, 10.0],
                [120.0, 40.0],
                [20.0, 40.0],
            ]
        );
    }

    #[test]
    fn parse_token_response() {
        let json = r#"{
            "access_token": "test_token_123",
            "expires_in": 2592000
        }"#;

        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.access_token.as_deref(), Some("test_token_123"));
        assert_eq!(resp.expires_in, Some(2592000));
        assert!(resp.error.is_none());
    }

    #[test]
    fn parse_token_error_response() {
        let json = r#"{
            "error": "invalid_client",
            "error_description": "Unknown client id"
        }"#;

        let resp: TokenResponse = serde_json::from_str(json).unwrap();
        assert!(resp.access_token.is_none());
        assert_eq!(resp.error.as_deref(), Some("invalid_client"));
    }
}
