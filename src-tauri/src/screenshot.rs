use std::io::Cursor;

use anyhow::{bail, Context};
use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use xcap::Monitor;

/// 截图数据大小上限（10 MB，base64 后约 13 MB）
const MAX_SCREENSHOT_BYTES: usize = 10 * 1024 * 1024;

/// 截取主显示器全屏，返回 PNG 字节
pub fn capture_primary_monitor_png() -> anyhow::Result<Vec<u8>> {
    let monitors = Monitor::all().context("获取显示器列表失败")?;
    let primary = monitors
        .into_iter()
        .filter_map(|m| match m.is_primary() {
            Ok(true) => Some(m),
            Ok(false) => None,
            Err(e) => {
                log::warn!("is_primary() 失败: {}", e);
                None
            }
        })
        .next()
        .context("未找到主显示器")?;

    let img = primary.capture_image().context("截图失败")?;
    encode_png(&img)
}

/// 截取指定区域，返回 PNG 字节
///
/// - `x`, `y`: 区域左上角的屏幕坐标
/// - `width`, `height`: 区域尺寸
pub fn capture_region_png(x: u32, y: u32, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    if width == 0 || height == 0 {
        bail!("截图区域尺寸不能为 0");
    }

    // 用区域中心点定位所在显示器（安全转换，防止 u32→i32 溢出）
    let cx = x as i64 + width as i64 / 2;
    let cy = y as i64 + height as i64 / 2;
    let cx_i32 = i32::try_from(cx).context("区域中心坐标 x 超出屏幕范围")?;
    let cy_i32 = i32::try_from(cy).context("区域中心坐标 y 超出屏幕范围")?;
    let monitor =
        Monitor::from_point(cx_i32, cy_i32).context("未找到包含目标区域的显示器")?;

    let img = monitor.capture_image().context("截图失败")?;

    // 裁剪到请求区域（坐标相对于显示器原点）
    let mon_x = monitor.x().unwrap_or(0) as u32;
    let mon_y = monitor.y().unwrap_or(0) as u32;
    let crop_x = x.saturating_sub(mon_x);
    let crop_y = y.saturating_sub(mon_y);

    // 防止 crop_imm 越界 panic：clamp 到实际图像尺寸
    let img_w = img.width();
    let img_h = img.height();
    let safe_crop_x = crop_x.min(img_w.saturating_sub(1));
    let safe_crop_y = crop_y.min(img_h.saturating_sub(1));
    let safe_w = width.min(img_w.saturating_sub(safe_crop_x));
    let safe_h = height.min(img_h.saturating_sub(safe_crop_y));
    if safe_w == 0 || safe_h == 0 {
        bail!("裁剪区域超出图像范围");
    }

    let cropped =
        image::imageops::crop_imm(&img, safe_crop_x, safe_crop_y, safe_w, safe_h).to_image();
    encode_png(&cropped)
}

fn encode_png(img: &image::RgbaImage) -> anyhow::Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    let encoder = PngEncoder::new(&mut buf);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .context("PNG 编码失败")?;
    let bytes = buf.into_inner();
    if bytes.len() > MAX_SCREENSHOT_BYTES {
        bail!(
            "截图数据过大 ({} bytes)，超过 {} MB 限制",
            bytes.len(),
            MAX_SCREENSHOT_BYTES / 1024 / 1024
        );
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_primary_monitor_returns_valid_png() {
        let png = capture_primary_monitor_png().expect("截图不应失败");
        // PNG 魔数
        assert!(png.len() > 8, "PNG 数据不应为空");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "应返回有效 PNG 格式");
    }

    #[test]
    fn test_capture_region_zero_size_rejected() {
        let err = capture_region_png(0, 0, 0, 100).unwrap_err();
        assert!(err.to_string().contains("不能为 0"));
    }

    #[test]
    fn test_capture_region_out_of_bounds_clamped() {
        // 远超屏幕范围的区域不应 panic，应返回错误或成功裁剪
        let result = capture_region_png(0, 0, 99999, 99999);
        // 可能成功（clamp 到屏幕大小）也可能 bail（裁剪后为 0）
        // 关键是不 panic
        let _ = result;
    }
}
