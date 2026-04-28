import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { translateText, type TranslationResult } from "../lib/translation-service";

export interface LiveTranscriptPayload {
  text: string;
  language: string | null;
  confidence: number;
  latency_ms: number;
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
  const [translation, setTranslation] = useState<TranslationResult | null>(null);
  const [source, setSource] = useState<"live" | "manual">("manual");
  const [error, setError] = useState<string | null>(null);

  // 监听 live-transcript 事件
  useEffect(() => {
    const unlisten = listen<LiveTranscriptPayload>("live-transcript", async (event) => {
      const { text, is_final } = event.payload;
      setTranscript(text);

      // 只翻译 is_final（避免频繁触发翻译请求）
      if (is_final && text.trim()) {
        try {
          setSource("live");
          setError(null);
          const result = await translateText(text);
          setTranslation(result);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        }
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
      setIsActive(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  // 停止实时翻译（自动切回 manual）
  const stop = useCallback(async () => {
    try {
      await invoke("stop_live_audio_translation");
      setIsActive(false);
      setSource("manual");
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
