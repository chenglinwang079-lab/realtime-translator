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
  const openSettings = useUiStore((s) => s.openSettings);

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

  const handlePin = useCallback(() => {
    setBubbleState("pinned");
    toggleSidebar();
  }, [setBubbleState, toggleSidebar]);

  const handleMinimize = useCallback(async () => {
    try {
      await getCurrentWindow().minimize();
    } catch {
      // ignore
    }
  }, []);

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
            onPin={handlePin}
            onMinimize={handleMinimize}
            onClose={handleClose}
            onOpenSidebar={handleOpenSidebar}
            onOpenSettings={openSettings}
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
            <button
              className="floating-bubble__settings-btn"
              onClick={openSettings}
              title="设置"
              type="button"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
              </svg>
              <span>设置</span>
            </button>
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
            onEngineSwitch={currentOriginal ? onRetry : undefined}
          />
        )}
      </GlassCard>
    </div>
  );
}
