import { useEffect, useRef, useCallback } from "react";
import { useTranslationStore } from "../stores/translationStore";
import { useUiStore } from "../stores/uiStore";
import {
  translate,
  startClipboardWatch,
  stopClipboardWatch,
  onClipboardChanged,
  showBubbleWindow,
  saveHistory,
  getHistory,
} from "../lib/tauri-bridge";
import { friendlyMessage } from "../lib/errorMessages";

/**
 * 剪贴板翻译管道 hook
 * 监听剪贴板变化 → 调用翻译 → 更新 store → 显示气泡
 */
export function useTranslationPipeline() {
  const lastTranslatedRef = useRef("");

  const setCurrentResult = useTranslationStore((s) => s.setCurrentResult);
  const setTranslating = useTranslationStore((s) => s.setTranslating);
  const setTranslateError = useTranslationStore((s) => s.setTranslateError);
  const addToHistory = useTranslationStore((s) => s.addToHistory);
  const loadHistory = useTranslationStore((s) => s.loadHistory);

  const setBubbleState = useUiStore((s) => s.setBubbleState);

  const handleTranslation = useCallback(
    async (text: string) => {
      // 去重
      if (text === lastTranslatedRef.current) return;
      lastTranslatedRef.current = text;

      // 合并为单次 store 更新，避免 4 次独立 set 导致多次 re-render
      useTranslationStore.setState({
        currentOriginal: text,
        currentResult: null,
        translateError: "",
        isTranslating: true,
      });
      setBubbleState("interactive");

      try {
        await showBubbleWindow();
        const result = await translate(text);
        setCurrentResult(result);
        const entry = {
          id: crypto.randomUUID(),
          originalText: text,
          translatedText: result.translatedText,
          sourceLang: result.sourceLang,
          targetLang: result.targetLang,
          engineId: result.engineId,
          timestamp: Date.now(),
          latencyMs: result.latencyMs,
        };
        addToHistory(entry);
        // 持久化到后端数据库
        saveHistory({
          ...entry,
          timestamp: new Date(entry.timestamp).toISOString(),
        }).catch((e) => console.error("保存历史失败:", e));
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setTranslateError(friendlyMessage(msg));
      } finally {
        setTranslating(false);
      }
    },
    [
      setCurrentResult,
      setTranslating,
      setTranslateError,
      addToHistory,
      setBubbleState,
    ]
  );

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    // 启动时从数据库加载历史记录
    getHistory(200)
      .then((entries) => {
        const converted = entries.map((e) => ({
          id: e.id,
          originalText: e.originalText,
          translatedText: e.translatedText,
          sourceLang: e.sourceLang,
          targetLang: e.targetLang,
          engineId: e.engineId,
          timestamp: new Date(e.timestamp).getTime(),
          latencyMs: e.latencyMs,
        }));
        loadHistory(converted);
      })
      .catch((e) => console.error("加载历史失败:", e));

    onClipboardChanged((event) => {
      handleTranslation(event.text);
    }).then((fn) => {
      unlisten = fn;
    });

    startClipboardWatch().catch((e) =>
      console.error("Failed to start clipboard watch:", e)
    );

    return () => {
      unlisten?.();
      stopClipboardWatch().catch(() => {});
    };
  }, [handleTranslation]);

  return {
    translate: handleTranslation,
    clearDedup: () => {
      lastTranslatedRef.current = "";
    },
  };
}
