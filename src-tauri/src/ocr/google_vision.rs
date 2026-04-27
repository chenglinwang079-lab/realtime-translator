use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;

use super::engine::{OcrEngine, OcrLevel, OcrResult, OcrTextBlock};
use super::group_words_to_lines;
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
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
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

    /// 将 bounding_poly 转换为原始多边形顶点（保留旋转信息）
    fn extract_polygon(poly: &BoundingPoly) -> Option<Vec<[f64; 2]>> {
        let valid: Vec<[f64; 2]> = poly
            .vertices
            .iter()
            .filter_map(|v| match (v.x, v.y) {
                (Some(x), Some(y)) => Some([x as f64, y as f64]),
                _ => None,
            })
            .collect();
        if valid.len() >= 2 {
            Some(valid)
        } else {
            None
        }
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

        let response = self
            .client
            .post(VISION_API_URL)
            .header("x-goog-api-key", &self.api_key)
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

        // 第一个元素是全文 + locale（原始全文由 line texts 重建，此处仅提取 locale）
        let language = annotations[0].locale.clone();

        // 后续元素是逐词/逐行文本块
        let words: Vec<OcrTextBlock> = annotations[1..]
            .iter()
            .map(|ann| {
                let bbox = ann
                    .bounding_poly
                    .as_ref()
                    .and_then(|poly| Self::convert_bbox(poly));
                let polygon = ann
                    .bounding_poly
                    .as_ref()
                    .and_then(|poly| Self::extract_polygon(poly));
                let font_size = bbox.map(|b| b[3]); // height = estimated font size

                OcrTextBlock {
                    text: ann.description.clone(),
                    bbox,
                    polygon,
                    confidence: None,
                    font_size,
                    level: OcrLevel::Word,
                }
            })
            .collect();

        // 行分组：将 word 按空间位置合并为 line
        let lines = group_words_to_lines(&words);

        // 无 bbox 的 word 保留（不丢词）
        let no_bbox_words: Vec<&OcrTextBlock> =
            words.iter().filter(|w| w.bbox.is_none()).collect();

        // full_text：line texts 用 \n 连接 + 无 bbox word texts 追加
        let mut full_text_parts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        for w in &no_bbox_words {
            full_text_parts.push(w.text.clone());
        }
        let full_text = full_text_parts.join("\n");

        // 合并 blocks：lines 在前，words 在后
        // 设计意图：lines 用于布局定位，words 用于逐词高亮。
        // 前端应按 level 字段区分使用，避免重复渲染。
        let mut blocks = lines;
        blocks.extend(words);

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

    #[test]
    fn extract_polygon_full() {
        let poly = BoundingPoly {
            vertices: vec![
                Vertex { x: Some(10), y: Some(20) },
                Vertex { x: Some(50), y: Some(20) },
                Vertex { x: Some(50), y: Some(50) },
                Vertex { x: Some(10), y: Some(50) },
            ],
        };
        let result = GoogleVisionEngine::extract_polygon(&poly);
        assert_eq!(
            result,
            Some(vec![
                [10.0, 20.0],
                [50.0, 20.0],
                [50.0, 50.0],
                [10.0, 50.0],
            ])
        );
    }

    #[test]
    fn extract_polygon_partial() {
        let poly = BoundingPoly {
            vertices: vec![
                Vertex { x: Some(10), y: None },
                Vertex { x: None, y: Some(20) },
            ],
        };
        let result = GoogleVisionEngine::extract_polygon(&poly);
        assert_eq!(result, None);
    }

    #[test]
    fn parse_response_enhanced_fields() {
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
        let ann = &resp.responses[0].text_annotations.as_ref().unwrap()[1];

        // 验证 extract_polygon
        let polygon = GoogleVisionEngine::extract_polygon(ann.bounding_poly.as_ref().unwrap());
        assert_eq!(
            polygon,
            Some(vec![
                [10.0, 20.0],
                [50.0, 20.0],
                [50.0, 50.0],
                [10.0, 50.0],
            ])
        );

        // 验证 bbox → font_size
        let bbox = GoogleVisionEngine::convert_bbox(ann.bounding_poly.as_ref().unwrap());
        assert_eq!(bbox, Some([10.0, 20.0, 40.0, 30.0]));
        assert_eq!(bbox.map(|b| b[3]), Some(30.0)); // font_size = height
    }
}
