pub mod engine;
pub mod engine_manager;
pub mod google_vision;

/// OCR API 错误 body 截断 + 清洗（共用工具函数，Google/Baidu/后续引擎复用）
pub fn truncate_error_body(body: &str, max_len: usize) -> String {
    let compressed: String = body.split_whitespace().collect::<Vec<&str>>().join(" ");
    let char_count = compressed.chars().count();
    if char_count > max_len {
        let truncated: String = compressed.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        compressed
    }
}
