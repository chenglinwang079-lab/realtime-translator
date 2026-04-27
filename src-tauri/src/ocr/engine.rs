use serde::{Deserialize, Serialize};

/// 单个识别文本块
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrTextBlock {
    /// 识别出的文本
    pub text: String,
    /// 边界框 [x, y, width, height]（像素坐标，可选）
    pub bbox: Option<[f64; 4]>,
    /// 置信度 0.0~1.0（可选）
    pub confidence: Option<f64>,
}

/// OCR 识别结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    /// 所有文本块（按阅读顺序排列）
    pub blocks: Vec<OcrTextBlock>,
    /// 合并后的完整文本（block 间用换行连接）
    pub full_text: String,
    /// 检测到的语言（如 "zh", "en", "ja"，可选）
    pub language: Option<String>,
    /// 使用的引擎 ID
    pub engine_id: String,
    /// 识别耗时（毫秒）
    pub latency_ms: u64,
}

/// OCR 引擎信息（用于状态展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrEngineInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
}

/// OCR 引擎 trait
///
/// 各平台/云端 OCR 实现此 trait。输入为图片原始字节（PNG/JPEG），
/// 输出为结构化的文本块列表。
#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    /// 引擎唯一标识（如 "google-vision", "baidu-ocr"）
    fn engine_id(&self) -> &str;

    /// 引擎显示名称
    fn engine_name(&self) -> &str;

    /// 引擎是否可用（如 API Key 是否已配置）
    fn is_available(&self) -> bool;

    /// 识别图片中的文字
    ///
    /// - `image_data`: 图片原始字节（PNG 或 JPEG 格式）
    /// - 返回结构化的 OCR 结果
    async fn recognize(&self, image_data: &[u8]) -> anyhow::Result<OcrResult>;

    /// 健康检查（发送测试请求验证连通性）
    ///
    /// 默认实现：用一个最小测试图片调用 recognize。
    /// 各引擎可覆盖以提供更精确的健康检查。
    ///
    /// **注意**：默认实现会调用付费 API（如 Google Vision / 百度 OCR）。
    /// 按量计费的引擎应覆盖此方法，改用免费的连通性检查（如 HTTP HEAD）。
    async fn health_check(&self) -> anyhow::Result<u64> {
        // 1x1 白色 PNG（最小有效图片）
        let test_png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, // IDAT chunk
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, // IEND chunk
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let result = self.recognize(test_png).await?;
        Ok(result.latency_ms)
    }
}
