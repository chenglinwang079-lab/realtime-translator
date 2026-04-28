import { invoke } from "@tauri-apps/api/core";

export interface TranslationResult {
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
}

/**
 * 共享翻译服务（手动和实时共用）
 */
export async function translateText(text: string): Promise<TranslationResult> {
  const result = await invoke<{
    translated: string;
    source_lang: string;
    target_lang: string;
    engine_id: string;
    latency_ms: number;
  }>("translate", { text });

  return {
    translatedText: result.translated,
    sourceLang: result.source_lang,
    targetLang: result.target_lang,
    engineId: result.engine_id,
    latencyMs: result.latency_ms,
  };
}
