import { describe, it, expect } from "vitest";
import { friendlyMessage } from "./errorMessages";

describe("friendlyMessage — translation prefixes", () => {
  it("maps TRANSLATE_INPUT", () => {
    expect(friendlyMessage("[TRANSLATE_INPUT] 翻译文本为空")).toBe(
      "请输入要翻译的文字"
    );
  });

  it("maps TRANSLATE_LENGTH", () => {
    expect(friendlyMessage("[TRANSLATE_LENGTH] 翻译文本过长")).toBe(
      "翻译文本过长，请缩短后重试"
    );
  });

  it("maps specific engine error before generic", () => {
    expect(
      friendlyMessage("[TRANSLATE] DeepL API 错误 (401): Invalid key")
    ).toBe("DeepL API Key 无效，请检查设置");
  });

  it("falls back to generic [TRANSLATE]", () => {
    expect(friendlyMessage("[TRANSLATE] some unknown error")).toBe(
      "翻译失败，请重试"
    );
  });

  it("maps normalized timeout", () => {
    expect(
      friendlyMessage("[TRANSLATE] 翻译请求超时，请稍后重试")
    ).toBe("翻译请求超时，请稍后重试");
  });
});
