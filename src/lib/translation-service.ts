import { invoke } from "@tauri-apps/api/core";

export interface TranslationResult {
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
}

/** 翻译超时时间（毫秒） */
const TRANSLATE_TIMEOUT_MS = 10000;

/** 最大重试次数 */
const MAX_RETRIES = 3;

/** 重试延迟（毫秒） */
const RETRY_DELAY_MS = 1000;

/**
 * 延迟指定时间
 */
function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * 带超时的 Promise
 */
function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error(`翻译超时 (${timeoutMs}ms)`)), timeoutMs)
    ),
  ]);
}

/**
 * 共享翻译服务（手动和实时共用）
 * 支持超时和重试机制
 */
export async function translateText(text: string): Promise<TranslationResult> {
  let lastError: Error | null = null;

  for (let attempt = 1; attempt <= MAX_RETRIES; attempt++) {
    try {
      const result = await withTimeout(
        invoke<{
          translated: string;
          source_lang: string;
          target_lang: string;
          engine_id: string;
          latency_ms: number;
        }>("translate", { text }),
        TRANSLATE_TIMEOUT_MS
      );

      return {
        translatedText: result.translated,
        sourceLang: result.source_lang,
        targetLang: result.target_lang,
        engineId: result.engine_id,
        latencyMs: result.latency_ms,
      };
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      console.warn(`[TranslationService] 翻译失败 (尝试 ${attempt}/${MAX_RETRIES}):`, lastError.message);

      // 如果不是最后一次尝试，等待后重试
      if (attempt < MAX_RETRIES) {
        await delay(RETRY_DELAY_MS * attempt); // 指数退避
      }
    }
  }

  // 所有重试都失败
  throw lastError || new Error("翻译失败");
}

