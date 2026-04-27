import { useCallback, useEffect } from "react";
import { GlassCard } from "./GlassCard";
import { TranslationResult as TranslationResultView } from "./TranslationResult";
import { BubbleActions } from "./BubbleActions";
import { useTranslationStore } from "../../stores/translationStore";
import { useUiStore } from "../../stores/uiStore";
import { hideBubbleWindow } from "../../lib/tauri-bridge";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./bubble.css";

export function FloatingBubble({ onRetry }: { onRetry?: () => void }) {
  const currentOriginal = useTranslationStore((s) => s.currentOriginal);
  const currentResult = useTranslationStore((s) => s.currentResult);
  const isTranslating = useTranslationStore((s) => s.isTranslating);
  const translateError = useTranslationStore((s) => s.translateError);
  const clearCurrent = useTranslationStore((s) => s.clearCurrent);

  const bubbleState = useUiStore((s) => s.bubbleState);
  const setBubbleState = useUiStore((s) => s.setBubbleState);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);

  const hasTranslation = currentOriginal.length > 0;

  const handleClose = useCallback(async () => {
    setBubbleState("dismissed");
    clearCurrent();
    try {
      await hideBubbleWindow();
    } catch {
      // ignore
    }
  }, [setBubbleState, clearCurrent]);

  const handleCopy = useCallback(() => {
    // 复制成功后的视觉反馈可后续添加
  }, []);

  const handlePin = useCallback(() => {
    setBubbleState("pinned");
    toggleSidebar();
  }, [setBubbleState, toggleSidebar]);

  const handleOpenSidebar = useCallback(() => {
    toggleSidebar();
  }, [toggleSidebar]);

  // 拖拽窗口
  const handleDragStart = useCallback(async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      await getCurrentWindow().startDragging();
    } catch {
      // ignore
    }
  }, []);

  // Esc 关闭气泡
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && bubbleState !== "dismissed") {
        e.preventDefault();
        handleClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [bubbleState, handleClose]);

  if (!hasTranslation && bubbleState === "dismissed") {
    return null;
  }

  return (
    <div className={`floating-bubble floating-bubble--${bubbleState}`}>
      {/* 气泡顶部：可拖拽区域 + 操作按钮 */}
      {hasTranslation && (
        <div className="floating-bubble__header">
          <div className="floating-bubble__drag-hint" onMouseDown={handleDragStart}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="9" cy="5" r="2" />
              <circle cx="15" cy="5" r="2" />
              <circle cx="9" cy="12" r="2" />
              <circle cx="15" cy="12" r="2" />
              <circle cx="9" cy="19" r="2" />
              <circle cx="15" cy="19" r="2" />
            </svg>
          </div>
          <BubbleActions
            translatedText={currentResult?.translatedText ?? ""}
            onCopy={handleCopy}
            onPin={handlePin}
            onClose={handleClose}
            onOpenSidebar={handleOpenSidebar}
          />
        </div>
      )}

      {/* 气泡内容 */}
      <GlassCard className="floating-bubble__card">
        {!hasTranslation ? (
          <div className="floating-bubble__empty" onMouseDown={handleDragStart}>
            <p className="floating-bubble__hint">
              复制文本即可自动翻译
            </p>
            <p className="floating-bubble__hint-sub">
              支持中英互译 · Ctrl+Shift+R 截图翻译 · Ctrl+Shift+T 手动翻译
            </p>
          </div>
        ) : (
          <TranslationResultView
            originalText={currentOriginal}
            translatedText={currentResult?.translatedText ?? ""}
            sourceLang={currentResult?.sourceLang ?? ""}
            targetLang={currentResult?.targetLang ?? ""}
            engineId={currentResult?.engineId ?? ""}
            latencyMs={currentResult?.latencyMs ?? 0}
            isTranslating={isTranslating}
            error={translateError}
            onRetry={translateError && currentOriginal ? onRetry : undefined}
          />
        )}
      </GlassCard>
    </div>
  );
}
