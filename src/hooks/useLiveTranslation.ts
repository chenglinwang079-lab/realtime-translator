import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { translate } from "../lib/tauri-bridge";

export interface LiveTranslationPayload {
  transcript_text: string;
  translated_text: string;
  source_language: string | null;
  target_language: string | null;
  is_final: boolean;
  chunk_id: number;
  timestamp_ms: number;
  duration_ms: number | null;
}

export interface LiveTranslationError {
  error: string;
  recoverable: boolean;
}

export interface LiveTranslationState {
  is_active: boolean;
  duration_ms: number;
}

export interface LiveSentence {
  transcript: string;
  translation: string;
  sourceLanguage?: string | null;
  targetLanguage?: string | null;
  timestamp: number;
}

const MAX_SENTENCES = 50;

export function useLiveTranslation() {
  const [isActive, setIsActive] = useState(false);
  const [currentTranscript, setCurrentTranscript] = useState("");
  const [sentences, setSentences] = useState<LiveSentence[]>([]);
  const [source, setSource] = useState<"live" | "manual">("manual");
  const [error, setError] = useState<string | null>(null);
  const [consolidatedTranslation, setConsolidatedTranslation] = useState<string | null>(null);
  const [isConsolidating, setIsConsolidating] = useState(false);

  // 监听 live-translation-result 事件
  useEffect(() => {
    const unlisten = listen<LiveTranslationPayload>("live-translation-result", (event) => {
      const { transcript_text, translated_text, is_final, timestamp_ms,
              source_language, target_language } = event.payload;

      if (!is_final) {
        // 中间结果：只更新当前句预览
        setCurrentTranscript(transcript_text);
        return;
      }

      // 最终结果：清空当前句，追加到列表
      setCurrentTranscript("");
      if (transcript_text.trim()) {
        setSource("live");
        setError(null);
        setSentences(prev => [...prev, {
          transcript: transcript_text,
          translation: translated_text || "",
          sourceLanguage: source_language,
          targetLanguage: target_language,
          timestamp: timestamp_ms,
        }].slice(-MAX_SENTENCES));
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 监听 live-translation-error 事件
  useEffect(() => {
    const unlisten = listen<LiveTranslationError>("live-translation-error", (event) => {
      setError(event.payload.error);
      // 不可恢复错误时清空当前句预览
      if (!event.payload.recoverable) {
        setCurrentTranscript("");
      }
      console.warn("[LiveTranslation] 错误:", event.payload.error);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 监听 live-translation-state-changed 事件
  useEffect(() => {
    const unlisten = listen<LiveTranslationState>("live-translation-state-changed", (event) => {
      setIsActive(event.payload.is_active);
      // 停止时清空当前句预览，切回 manual
      if (!event.payload.is_active) {
        setCurrentTranscript("");
        setSource("manual");
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 开始实时翻译（新会话，清空历史）
  const start = useCallback(async () => {
    try {
      setSentences([]);
      setCurrentTranscript("");
      setConsolidatedTranslation(null);
      setError(null);
      await invoke("start_live_audio_translation");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // 停止实时翻译
  const stop = useCallback(async () => {
    try {
      setCurrentTranscript("");
      await invoke("stop_live_audio_translation");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // 整合翻译：将所有逐句原文合并后做一次完整翻译
  const consolidate = useCallback(async () => {
    if (sentences.length === 0) return;
    const fullText = sentences.map(s => s.transcript).join("\n");
    setError(null);
    setIsConsolidating(true);
    try {
      const result = await translate(fullText);
      setConsolidatedTranslation(result.translatedText);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsConsolidating(false);
    }
  }, [sentences]);

  // 清除会话，回到手动模式
  const clearSession = useCallback(async () => {
    if (isActive) {
      try {
        await invoke("stop_live_audio_translation");
      } catch {
        // ignore
      }
    }
    setSentences([]);
    setCurrentTranscript("");
    setConsolidatedTranslation(null);
    setError(null);
    setSource("manual");
  }, [isActive]);

  return {
    isActive,
    currentTranscript,
    sentences,
    source,
    error,
    consolidatedTranslation,
    isConsolidating,
    consolidate,
    start,
    stop,
    clearSession,
  };
}
