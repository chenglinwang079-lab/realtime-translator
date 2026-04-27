pub mod engine;
pub mod engine_manager;
pub mod google_vision;
pub mod baidu_ocr;

use engine::{OcrLevel, OcrTextBlock};

/// 行内拼接间距阈值：间距 > 行高 × 此值时加空格
const SPACE_GAP_RATIO: f64 = 0.3;

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

/// 将 word 级别的文本块按空间位置分组为 line 级别
///
/// 分组依据：垂直方向上的重叠度。同一行的文字具有相似的
/// y-center 和高度，重叠超过 50% 则归为同一行。
///
/// 只处理有 bbox 的 word；无 bbox 的 word 被过滤（调用方负责保留）。
///
/// `full_text` 构建契约：
/// - 有 bbox 的 word → 分组为 line → 出现在 line texts 中
/// - 无 bbox 的 word → 保留在 word blocks 中 → 追加到 full_text
/// - 两者互斥，不会重复。新引擎实现应遵守此契约
pub fn group_words_to_lines(words: &[OcrTextBlock]) -> Vec<OcrTextBlock> {
    let mut with_bbox: Vec<&OcrTextBlock> = words.iter().filter(|w| w.bbox.is_some()).collect();

    if with_bbox.is_empty() {
        return vec![];
    }

    // 按 y-center 排序
    with_bbox.sort_by(|a, b| {
        let ay = a.bbox.unwrap()[1] + a.bbox.unwrap()[3] / 2.0;
        let by = b.bbox.unwrap()[1] + b.bbox.unwrap()[3] / 2.0;
        ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 按垂直重叠分组
    let mut groups: Vec<Vec<&OcrTextBlock>> = vec![];
    let mut current_group: Vec<&OcrTextBlock> = vec![with_bbox[0]];

    for word in &with_bbox[1..] {
        let wb = word.bbox.unwrap();
        let word_top = wb[1];
        let word_bottom = wb[1] + wb[3];

        let group_min_y = current_group
            .iter()
            .map(|w| w.bbox.unwrap()[1])
            .fold(f64::INFINITY, f64::min);
        let group_max_y = current_group
            .iter()
            .map(|w| w.bbox.unwrap()[1] + w.bbox.unwrap()[3])
            .fold(f64::NEG_INFINITY, f64::max);

        let overlap = (word_bottom.min(group_max_y) - word_top.max(group_min_y)).max(0.0);

        let group_avg_height = current_group
            .iter()
            .map(|w| w.bbox.unwrap()[3])
            .sum::<f64>()
            / current_group.len() as f64;
        let min_height = wb[3].min(group_avg_height);
        if overlap > min_height * 0.5 {
            current_group.push(word);
        } else {
            groups.push(current_group);
            current_group = vec![word];
        }
    }
    groups.push(current_group);

    // 合并每组为 line block
    groups.iter().map(|group| merge_group_to_line(group)).collect()
}

/// 将同一行的多个 word 合并为一个 line block
fn merge_group_to_line(group: &[&OcrTextBlock]) -> OcrTextBlock {
    // 按 x 排序（阅读顺序）
    let mut sorted = group.to_vec();
    sorted.sort_by(|a, b| {
        let ax = a.bbox.unwrap()[0];
        let bx = b.bbox.unwrap()[0];
        ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
    });

    // 智能拼接：基于间距判断是否加空格
    let mut text = String::new();
    for (i, word) in sorted.iter().enumerate() {
        if i > 0 {
            if should_insert_space(sorted[i - 1], word) {
                text.push(' ');
            }
        }
        text.push_str(&word.text);
    }

    // Union AABB
    let min_x = sorted
        .iter()
        .map(|w| w.bbox.unwrap()[0])
        .fold(f64::INFINITY, f64::min);
    let min_y = sorted
        .iter()
        .map(|w| w.bbox.unwrap()[1])
        .fold(f64::INFINITY, f64::min);
    let max_x = sorted
        .iter()
        .map(|w| w.bbox.unwrap()[0] + w.bbox.unwrap()[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = sorted
        .iter()
        .map(|w| w.bbox.unwrap()[1] + w.bbox.unwrap()[3])
        .fold(f64::NEG_INFINITY, f64::max);
    let bbox = Some([min_x, min_y, max_x - min_x, max_y - min_y]);

    // 合并 polygon（轴对齐近似）
    let polygon = merge_polygons(&sorted);

    // 平均 font_size
    let font_sizes: Vec<f64> = sorted.iter().filter_map(|w| w.font_size).collect();
    let font_size = if font_sizes.is_empty() {
        None
    } else {
        Some(font_sizes.iter().sum::<f64>() / font_sizes.len() as f64)
    };

    // 平均 confidence（跳过 None）
    let confidences: Vec<f64> = sorted.iter().filter_map(|w| w.confidence).collect();
    let confidence = if confidences.is_empty() {
        None
    } else {
        Some(confidences.iter().sum::<f64>() / confidences.len() as f64)
    };

    OcrTextBlock {
        text,
        bbox,
        polygon,
        confidence,
        font_size,
        level: OcrLevel::Line,
    }
}

/// 合并多个 word 的 polygon 为 line 级轴对齐包围矩形
///
/// 注意：这是近似值，不保留旋转信息。
fn merge_polygons(words: &[&OcrTextBlock]) -> Option<Vec<[f64; 2]>> {
    let all_vertices: Vec<[f64; 2]> = words
        .iter()
        .filter_map(|w| w.polygon.as_ref())
        .flatten()
        .copied()
        .collect();

    if all_vertices.is_empty() {
        return None;
    }

    let min_x = all_vertices
        .iter()
        .map(|v| v[0])
        .fold(f64::INFINITY, f64::min);
    let min_y = all_vertices
        .iter()
        .map(|v| v[1])
        .fold(f64::INFINITY, f64::min);
    let max_x = all_vertices
        .iter()
        .map(|v| v[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = all_vertices
        .iter()
        .map(|v| v[1])
        .fold(f64::NEG_INFINITY, f64::max);

    Some(vec![
        [min_x, min_y],
        [max_x, min_y],
        [max_x, max_y],
        [min_x, max_y],
    ])
}

/// 判断两个相邻 word 之间是否应插入空格
///
/// 基于 bbox 间距与行高比例判断：间距 > 行高 × SPACE_GAP_RATIO 时加空格。
/// CJK 字符间间距极小，不加空格；拉丁语系单词间有明显间隔，加空格。
pub fn should_insert_space(left: &OcrTextBlock, right: &OcrTextBlock) -> bool {
    let left_bbox = match left.bbox {
        Some(b) => b,
        None => return false,
    };
    let right_bbox = match right.bbox {
        Some(b) => b,
        None => return false,
    };

    let gap = right_bbox[0] - (left_bbox[0] + left_bbox[2]);
    let avg_height = ((left_bbox[3] + right_bbox[3]) / 2.0).max(1.0);

    gap > avg_height * SPACE_GAP_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrTextBlock {
        OcrTextBlock {
            text: text.to_string(),
            bbox: Some([x, y, w, h]),
            polygon: Some(vec![
                [x, y],
                [x + w, y],
                [x + w, y + h],
                [x, y + h],
            ]),
            confidence: None,
            font_size: Some(h),
            level: OcrLevel::Word,
        }
    }

    #[test]
    fn group_single_word() {
        let words = vec![make_word("Hello", 10.0, 20.0, 50.0, 16.0)];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[0].level, OcrLevel::Line);
    }

    #[test]
    fn group_two_words_same_line() {
        let words = vec![
            make_word("Hello", 10.0, 20.0, 50.0, 16.0),
            make_word("World", 70.0, 22.0, 50.0, 16.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello World");
    }

    #[test]
    fn group_two_words_different_lines() {
        let words = vec![
            make_word("Line1", 10.0, 20.0, 50.0, 16.0),
            make_word("Line2", 10.0, 100.0, 50.0, 16.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "Line1");
        assert_eq!(lines[1].text, "Line2");
    }

    #[test]
    fn group_mixed_font_sizes() {
        let words = vec![
            make_word("Big", 10.0, 20.0, 50.0, 24.0),
            make_word("Small", 70.0, 26.0, 40.0, 12.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        // font_size = average of 24 and 12 = 18
        assert!((lines[0].font_size.unwrap() - 18.0).abs() < 0.01);
    }

    #[test]
    fn group_empty_bboxes() {
        let words = vec![OcrTextBlock {
            text: "no-bbox".to_string(),
            bbox: None,
            polygon: None,
            confidence: None,
            font_size: None,
            level: OcrLevel::Word,
        }];
        let lines = group_words_to_lines(&words);
        assert!(lines.is_empty());
    }

    #[test]
    fn group_preserves_x_order() {
        let words = vec![
            make_word("middle", 100.0, 20.0, 50.0, 16.0),
            make_word("left", 10.0, 20.0, 50.0, 16.0),
            make_word("right", 200.0, 20.0, 50.0, 16.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "left middle right");
    }

    #[test]
    fn join_cjk_no_space() {
        // CJK 字符：间距极小（gap=2，行高=16，阈值=4.8）
        let words = vec![
            make_word("你好", 10.0, 20.0, 32.0, 16.0),
            make_word("世界", 44.0, 20.0, 32.0, 16.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "你好世界");
    }

    #[test]
    fn join_latin_with_space() {
        // Latin words: gap=10, height=16, threshold=4.8 → insert space
        let words = vec![
            make_word("Hello", 10.0, 20.0, 50.0, 16.0),
            make_word("World", 70.0, 20.0, 50.0, 16.0),
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Hello World");
    }

    #[test]
    fn group_mixed_size_same_line() {
        // 大字+小字同行：y-center 接近，重叠足够
        let words = vec![
            make_word("Title", 10.0, 10.0, 80.0, 32.0),  // y-center = 26
            make_word("sub", 100.0, 22.0, 40.0, 12.0),   // y-center = 28
        ];
        let lines = group_words_to_lines(&words);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Title sub");
    }
}
