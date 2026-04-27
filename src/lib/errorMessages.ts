/**
 * Rust 错误前缀 → 用户友好中文提示
 *
 * 前缀顺序：更具体的在前，通用的在后。
 * 例如 "[OCR] Google Vision API 错误 (403)" 必须在 "[OCR]" 之前，
 * 否则通用前缀会吃掉精确匹配。
 */
const PREFIX_MAP: [string, string][] = [
  ["[SCREENSHOT_TIMEOUT]", "截图超时，请重试"],
  ["[SCREENSHOT_TASK]", "截图任务失败，请重试"],
  ["[SCREENSHOT]", "截图失败，请重试"],
  ["[OCR_TIMEOUT]", "OCR 识别超时，请重试"],
  ["[OCR_INPUT]", "截图数据异常，请重试"],
  ["[OCR_ENGINE]", "OCR 引擎未配置，请在设置中配置 API Key"],
  ["[OCR_FALLBACK]", "所有 OCR 引擎均失败，请检查设置"],
  ["[OCR] Google Vision API 错误 (403)", "API Key 无效或权限不足"],
  ["[OCR] Google Vision API 错误 (429)", "API 调用次数超限，请稍后重试"],
  ["[OCR] Google Vision API 错误 (5", "OCR 服务暂时不可用，请稍后重试"],
  ["[OCR] Google Vision API 请求失败", "网络连接失败，请检查网络"],
  // 百度 OCR 按 error_code 精确映射（110/111 根因通常是凭据问题，文案聚焦凭据检查）
  ["[OCR] 百度 OCR 错误 (code=100)", "请求参数错误，请重试"],
  ["[OCR] 百度 OCR 错误 (code=110)", "百度 OCR 认证失败，请检查 API Key 和 Secret Key"],
  ["[OCR] 百度 OCR 错误 (code=111)", "百度 OCR 认证失败，请检查 API Key 和 Secret Key"],
  ["[OCR] 百度 OCR 错误 (code=216100)", "百度 OCR 配额已用完，请升级套餐或稍后重试"],
  ["[OCR] 百度 OCR Token 获取失败", "百度 OCR 认证失败，请检查 API Key 和 Secret Key"],
  ["[OCR] 百度 OCR 请求失败", "网络连接失败，请检查网络"],
  ["[OCR] 百度 OCR 错误", "百度 OCR 识别失败，请重试"],
  ["[OCR]", "OCR 识别失败，请重试"],
];

/** 兼容无前缀的旧错误（过渡期） */
const LEGACY_MAP: [RegExp, string][] = [
  [/截图区域尺寸不能为 0/, "截图区域太小，请重新框选"],
  [/区域中心坐标.*超出屏幕范围/, "截图区域超出屏幕范围"],
  [/GOOGLE_VISION_API_KEY/, "Google Vision API Key 未配置"],
];

/**
 * 将 Rust 原始错误字符串映射为用户友好的中文提示。
 * 优先匹配 `[PREFIX]` 前缀，其次匹配正则，最后截断兜底。
 */
export function friendlyMessage(raw: string): string {
  for (const [prefix, friendly] of PREFIX_MAP) {
    if (raw.startsWith(prefix)) return friendly;
  }
  for (const [pattern, friendly] of LEGACY_MAP) {
    if (pattern.test(raw)) return friendly;
  }
  return `操作失败：${raw.slice(0, 80)}`;
}
