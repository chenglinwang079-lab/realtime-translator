use std::time::Instant;

use anyhow::{bail, Context};
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;

use super::engine::{OcrEngine, OcrResult, OcrTextBlock};
use super::truncate_error_body;

const VISION_API_URL: &str = "https://vision.googleapis.com/v1/images:annotate";
const FEATURE_TYPE: &str = "TEXT_DETECTION";

// --- 私有响应类型 ---

#[derive(Deserialize)]
struct VisionResponse {
    responses: Vec<VisionAnnotateResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisionAnnotateResponse {
    text_annotations: Option<Vec<TextAnnotation>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextAnnotation {
    description: String,
    locale: Option<String>,
    bounding_poly: Option<BoundingPoly>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoundingPoly {
    vertices: Vec<Vertex>,
}

#[derive(Deserialize)]
struct Vertex {
    x: Option<i64>,
    y: Option<i64>,
}

// --- GoogleVisionEngine ---

pub struct GoogleVisionEngine {
    api_key: String,
    client: Client,
}

impl GoogleVisionEngine {
    /// 从环境变量创建（fallback）
    pub fn new() -> anyhow::Result<Self> {
        let api_key = std::env::var("GOOGLE_VISION_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            bail!("GOOGLE_VISION_API_KEY not configured");
        }
        Self::new_with_key(api_key)
    }

    /// 从指定的 key 创建
    pub fn new_with_key(api_key: String) -> anyhow::Result<Self> {
        let client = Client::builder()
            .use_rustls_tls()
            .build()
            .context("创建 HTTP client 失败")?;
        Ok(Self { api_key, client })
    }

    /// 将 bounding_poly 转换为 [x, y, width, height]
    fn convert_bbox(poly: &BoundingPoly) -> Option<[f64; 4]> {
        let valid: Vec<(i64, i64)> = poly
            .vertices
            .iter()
            .filter_map(|v| match (v.x, v.y) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            })
            .collect();

        if valid.len() < 2 {
            return None;
        }

        let min_x = valid.iter().map(|(x, _)| *x).min().unwrap_or(0);
        let min_y = valid.iter().map(|(_, y)| *y).min().unwrap_or(0);
        let max_x = valid.iter().map(|(x, _)| *x).max().unwrap_or(0);
        let max_y = valid.iter().map(|(_, y)| *y).max().unwrap_or(0);

        Some([
            min_x as f64,
            min_y as f64,
            (max_x - min_x) as f64,
            (max_y - min_y) as f64,
        ])
    }
}

#[async_trait::async_trait]
impl OcrEngine for GoogleVisionEngine {
    fn engine_id(&self) -> &str {
        "google-vision"
    }

    fn engine_name(&self) -> &str {
        "Google Cloud Vision"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn recognize(&self, image_data: &[u8]) -> anyhow::Result<OcrResult> {
        let start = Instant::now();

        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_data);

        let body = serde_json::json!({
            "requests": [{
                "image": { "content": image_base64 },
                "features": [{ "type": FEATURE_TYPE }]
            }]
        });

        let url = format!("{}?key={}", VISION_API_URL, self.api_key);

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("Google Vision API 请求失败")?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .context("读取 Google Vision 响应失败")?;

        if !status.is_success() {
            bail!(
                "Google Vision API 错误 ({}): {}",
                status,
                truncate_error_body(&response_text, 200)
            );
        }

        let vision_response: VisionResponse =
            serde_json::from_str(&response_text).context("解析 Google Vision 响应失败")?;

        let annotate = vision_response.responses.into_iter().next();

        let annotations = annotate
            .and_then(|a| a.text_annotations)
            .unwrap_or_default();

        let latency_ms = start.elapsed().as_millis() as u64;

        // 空结果
        if annotations.is_empty() {
            return Ok(OcrResult {
                blocks: vec![],
                full_text: String::new(),
                language: None,
                engine_id: self.engine_id().to_string(),
                latency_ms,
            });
        }

        // 第一个元素是全文 + locale
        let full_text = annotations[0].description.clone();
        let language = annotations[0].locale.clone();

        // 后续元素是逐词/逐行文本块
        let blocks: Vec<OcrTextBlock> = annotations[1..]
            .iter()
            .map(|ann| OcrTextBlock {
                text: ann.description.clone(),
                bbox: ann
                    .bounding_poly
                    .as_ref()
                    .and_then(|poly| Self::convert_bbox(poly)),
                confidence: None,
            })
            .collect();

        Ok(OcrResult {
            blocks,
            full_text,
            language,
            engine_id: self.engine_id().to_string(),
            latency_ms,
        })
    }

    /// 覆盖默认实现：仅检查 API Key 是否已配置，不发 API 请求
    async fn health_check(&self) -> anyhow::Result<u64> {
        if self.api_key.is_empty() {
            bail!("Google Vision API Key 未配置");
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_response() {
        let json = r#"{
            "responses": [{
                "textAnnotations": [
                    {
                        "description": "Hello World",
                        "locale": "en",
                        "boundingPoly": {
                            "vertices": [
                                {"x": 10, "y": 20},
                                {"x": 100, "y": 20},
                                {"x": 100, "y": 50},
                                {"x": 10, "y": 50}
                            ]
                        }
                    },
                    {
                        "description": "Hello",
                        "boundingPoly": {
                            "vertices": [
                                {"x": 10, "y": 20},
                                {"x": 50, "y": 20},
                                {"x": 50, "y": 50},
                                {"x": 10, "y": 50}
                            ]
                        }
                    },
                    {
                        "description": "World",
                        "boundingPoly": {
                            "vertices": [
                                {"x": 60, "y": 20},
                                {"x": 100, "y": 20},
                                {"x": 100, "y": 50},
                                {"x": 60, "y": 50}
                            ]
                        }
                    }
                ]
            }]
        }"#;

        let resp: VisionResponse = serde_json::from_str(json).unwrap();
        let annotations = resp.responses[0].text_annotations.as_ref().unwrap();

        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].description, "Hello World");
        assert_eq!(annotations[0].locale.as_deref(), Some("en"));

        // 验证 bbox 转换
        let bbox = GoogleVisionEngine::convert_bbox(
            annotations[1].bounding_poly.as_ref().unwrap(),
        );
        assert_eq!(bbox, Some([10.0, 20.0, 40.0, 30.0]));
    }

    #[test]
    fn parse_empty_response() {
        let json = r#"{"responses": [{}]}"#;
        let resp: VisionResponse = serde_json::from_str(json).unwrap();
        let annotations = resp.responses[0].text_annotations.as_ref();
        assert!(annotations.is_none());
    }

    #[test]
    fn parse_missing_vertices() {
        let json = r#"{
            "responses": [{
                "textAnnotations": [
                    { "description": "full", "locale": "zh" },
                    {
                        "description": "partial",
                        "boundingPoly": {
                            "vertices": [
                                {"x": 10},
                                {"y": 20}
                            ]
                        }
                    }
                ]
            }]
        }"#;

        let resp: VisionResponse = serde_json::from_str(json).unwrap();
        let ann = &resp.responses[0].text_annotations.as_ref().unwrap()[1];
        let bbox = GoogleVisionEngine::convert_bbox(ann.bounding_poly.as_ref().unwrap());
        // 只有 1 个有效顶点 (10, _) 和 (_, 20) 都不完整 → None
        assert_eq!(bbox, None);
    }
}
