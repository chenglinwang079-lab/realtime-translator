import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

export function useLiveTranslation() {
  const [isActive, setIsActive] = useState(false);
  const [transcript, setTranscript] = useState("");
  const [translation, setTranslation] = useState<string | null>(null);
  const [source, setSource] = useState<"live" | "manual">("manual");
  const [error, setError] = useState<string | null>(null);

  // 监听 live-translation-result 事件
  useEffect(() => {
    const unlisten = listen<LiveTranslationPayload>("live-translation-result", (event) => {
      const { transcript_text, translated_text, is_final } = event.payload;

      // 显示原文
      setTranscript(transcript_text);

      // 只处理最终结果的译文
      if (is_final && translated_text.trim()) {
        setSource("live");
        setTranslation(translated_text);
        setError(null);
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
      // 停止时切回 manual
      if (!event.payload.is_active) {
        setSource("manual");
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // 开始实时翻译
  const start = useCallback(async () => {
    try {
      setError(null);
      await invoke("start_live_audio_translation");
      // 不在这里设置 isActive，完全依赖 live-translation-state-changed 事件
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // 停止实时翻译（自动切回 manual）
  const stop = useCallback(async () => {
    try {
      await invoke("stop_live_audio_translation");
      // 不在这里设置 isActive 和 source，完全依赖事件
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  return {
    isActive,
    transcript,
    translation,
    source,
    error,
    start,
    stop,
  };
}
