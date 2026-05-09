import { useCallback, useEffect, useRef, useState } from "react";
import { TextInput } from "./TextInput";
import { LanguagePair } from "./LanguagePair";
import { TranslationOutput } from "./TranslationOutput";
import { useTranslationStore } from "../../stores/translationStore";
import { useUiStore } from "../../stores/uiStore";
import {
  translate,
  setWindowSize,
  getSelectedText,
  onUiaTextCaptured,
  showRegionSelector,
  type UiaTextCapturedEvent,
} from "../../lib/tauri-bridge";
import { useLiveTranslation } from "../../hooks/useLiveTranslation";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./sidebar.css";

const BUBBLE_WIDTH = 400;
const SIDEBAR_DEFAULT_WIDTH = 360;

export function Sidebar() {
  const sidebarVisible = useUiStore((s) => s.sidebarVisible);
  const setSidebarVisible = useUiStore((s) => s.setSidebarVisible);
  const ocrProcessing = useUiStore((s) => s.ocrProcessing);

  const currentOriginal = useTranslationStore((s) => s.currentOriginal);
  const currentResult = useTranslationStore((s) => s.currentResult);
  const isTranslating = useTranslationStore((s) => s.isTranslating);
  const translateError = useTranslationStore((s) => s.translateError);
  const setCurrentOriginal = useTranslationStore((s) => s.setCurrentOriginal);
  const setCurrentResult = useTranslationStore((s) => s.setCurrentResult);
  const setTranslating = useTranslationStore((s) => s.setTranslating);
  const setTranslateError = useTranslationStore((s) => s.setTranslateError);

  const screenOcrBusy = ocrProcessing || isTranslating;

  const [inputText, setInputText] = useState(currentOriginal);
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("zh");

  const sidebarRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(SIDEBAR_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = useState(false);
  const MIN_WIDTH = 300;
  const MAX_WIDTH = 600;

  const [uiaError, setUiaError] = useState("");

  const {
    isActive: isLiveActive,
    currentTranscript,
    sentences,
    error: liveError,
    consolidatedTranslation,
    isConsolidating,
    consolidate,
    start: startLiveTranslation,
    stop: stopLiveTranslation,
    clearSession,
  } = useLiveTranslation();
  const [isGrabbing, setIsGrabbing] = useState(false);

  const sentencesRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (sentencesRef.current) {
      sentencesRef.current.scrollTop = sentencesRef.current.scrollHeight;
    }
  }, [sentences]);

  // 动态管理侧边栏高度，确保 overflow 在 WebView2 中可靠工作
  useEffect(() => {
    const updateHeight = () => {
      if (sidebarRef.current) {
        sidebarRef.current.style.height = `${window.innerHeight}px`;
      }
    };
    updateHeight();
    window.addEventListener("resize", updateHeight);
    return () => window.removeEventListener("resize", updateHeight);
  }, []);

  // 模式判断：实时模式下隐藏手动翻译区
  const isLiveMode = isLiveActive || sentences.length > 0;

  useEffect(() => {
    const adjustWidth = async () => {
      try {
        const win = getCurrentWindow();
        const size = await win.innerSize();
        const currentHeight = size.height;
        if (sidebarVisible) {
          await setWindowSize(BUBBLE_WIDTH + width, currentHeight);
        } else {
          await setWindowSize(BUBBLE_WIDTH, currentHeight);
        }
      } catch {
        // ignore
      }
    };
    adjustWidth();
  }, [sidebarVisible, width]);

  useEffect(() => {
    if (currentOriginal && currentOriginal !== inputText) {
      setInputText(currentOriginal);
    }
  }, [currentOriginal]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    onUiaTextCaptured((event: UiaTextCapturedEvent) => {
      if (cancelled) return;
      if (event.text) {
        setInputText(event.text);
        setCurrentOriginal(event.text);
        setUiaError("");
      } else if (event.error) {
        setUiaError(event.error);
      }
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [setCurrentOriginal]);

  const handleTranslate = useCallback(async () => {
    const text = inputText.trim();
    if (!text) return;
    setCurrentOriginal(text);
    setTranslating(true);
    setTranslateError("");
    try {
      const result = await translate(text);
      setCurrentResult(result);
    } catch (err) {
      setTranslateError(err instanceof Error ? err.message : String(err));
    } finally {
      setTranslating(false);
    }
  }, [inputText, setCurrentOriginal, setTranslating, setTranslateError, setCurrentResult]);

  const handleSwapLanguages = useCallback(() => {
    if (sourceLang === "auto") return;
    const newSource = targetLang;
    const newTarget = sourceLang;
    setSourceLang(newSource);
    setTargetLang(newTarget);
    if (currentResult?.translatedText) {
      setInputText(currentResult.translatedText);
      setCurrentOriginal(currentResult.translatedText);
    }
  }, [sourceLang, targetLang, currentResult, setCurrentOriginal]);

  const handleClose = useCallback(() => {
    setSidebarVisible(false);
  }, [setSidebarVisible]);

  const handleResizeStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setIsResizing(true);
      const startX = e.clientX;
      const startWidth = width;
      const handleMouseMove = (e: MouseEvent) => {
        const delta = startX - e.clientX;
        const newWidth = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startWidth + delta));
        setWidth(newWidth);
      };
      const handleMouseUp = () => {
        setIsResizing(false);
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
      };
      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    [width]
  );

  const handleResizeKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const step = e.shiftKey ? 50 : 10;
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setWidth(Math.max(MIN_WIDTH, width - step));
      } else if (e.key === "ArrowRight") {
        e.preventDefault();
        setWidth(Math.min(MAX_WIDTH, width + step));
      } else if (e.key === "Home") {
        e.preventDefault();
        setWidth(MIN_WIDTH);
      } else if (e.key === "End") {
        e.preventDefault();
        setWidth(MAX_WIDTH);
      }
    },
    [width]
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

  const handleGrabText = useCallback(async () => {
    if (isGrabbing) return;
    setIsGrabbing(true);
    setUiaError("");
    try {
      const selection = await getSelectedText();
      if (selection?.text) {
        setInputText(selection.text);
        setCurrentOriginal(selection.text);
      } else {
        setUiaError("未检测到选中文本（目标应用可能不支持 TextPattern）");
      }
    } catch (err) {
      setUiaError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsGrabbing(false);
    }
  }, [setCurrentOriginal, isGrabbing]);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      const files = Array.from(e.dataTransfer.files);
      const textFile = files.find(
        (f) =>
          f.type.startsWith("text/") ||
          f.name.endsWith(".txt") ||
          f.name.endsWith(".md") ||
          f.name.endsWith(".csv") ||
          f.name.endsWith(".json") ||
          f.name.endsWith(".log")
      );
      if (textFile) {
        if (textFile.size > 100 * 1024) {
          setTranslateError("文件过大，请拖入 100KB 以内的文本文件");
          return;
        }
        const reader = new FileReader();
        reader.onload = async (event) => {
          const text = (event.target?.result as string)?.trim();
          if (text) {
            setInputText(text);
            setCurrentOriginal(text);
            setTranslating(true);
            setTranslateError("");
            try {
              const result = await translate(text);
              setCurrentResult(result);
            } catch (err) {
              setTranslateError(err instanceof Error ? err.message : String(err));
            } finally {
              setTranslating(false);
            }
          }
        };
        reader.readAsText(textFile);
      }
    },
    [setCurrentOriginal, setTranslating, setTranslateError, setCurrentResult]
  );

  if (!sidebarVisible) {
    return null;
  }

  return (
    <div
      ref={sidebarRef}
      className={`sidebar ${isResizing ? "sidebar--resizing" : ""} ${isLiveMode ? "sidebar--live-mode" : ""}`}
      style={{ width: `${width}px` }}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {/* 拖拽调整手柄 */}
      <div
        className="sidebar__resize-handle"
        role="separator"
        aria-orientation="vertical"
        aria-label="调整侧边栏宽度"
        aria-valuemin={MIN_WIDTH}
        aria-valuemax={MAX_WIDTH}
        aria-valuenow={width}
        tabIndex={0}
        onMouseDown={handleResizeStart}
        onKeyDown={handleResizeKeyDown}
      />

      {/* 侧边栏头部 */}
      <div className="sidebar__header">
        <div className="sidebar__header-left">
          {isLiveMode && (
            <button
              className="sidebar__back"
              onClick={clearSession}
              title="返回手动翻译"
              type="button"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <polyline points="15 18 9 12 15 6" />
              </svg>
            </button>
          )}
          <h2 className="sidebar__title">{isLiveMode ? "实时翻译" : "翻译"}</h2>
        </div>
        <button
          className="sidebar__close"
          onClick={handleClose}
          title="关闭侧边栏"
          type="button"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      {/* 手动翻译区 — 正常模式整体滚动 */}
      {!isLiveMode && (
        <div className="sidebar__normal-scroll">
          <LanguagePair
            sourceLang={sourceLang}
            targetLang={targetLang}
            onSourceLangChange={setSourceLang}
            onTargetLangChange={setTargetLang}
            onSwap={handleSwapLanguages}
          />

          <div className="sidebar__input-section">
            <TextInput
              value={inputText}
              onChange={setInputText}
              onTranslate={handleTranslate}
              disabled={isTranslating}
            />
          </div>

          <div className="sidebar__actions-bar">
            <button
              className="sidebar__translate-btn"
              onClick={handleTranslate}
              disabled={!inputText.trim() || isTranslating}
              type="button"
            >
              {isTranslating ? (
                <span className="sidebar__translate-btn-loading">
                  <span className="dot-pulse" />
                  翻译中
                </span>
              ) : (
                "翻译"
              )}
            </button>

            <div className="sidebar__tool-btns">
              <button
                className={`sidebar__tool-btn ${isGrabbing ? "sidebar__tool-btn--active" : ""}`}
                onClick={handleGrabText}
                disabled={isGrabbing}
                type="button"
                title="抓取文本 (Ctrl+Shift+G)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5" />
                </svg>
              </button>
              <button
                className={`sidebar__tool-btn ${ocrProcessing ? "sidebar__tool-btn--active" : ""}`}
                onClick={() => showRegionSelector()}
                disabled={screenOcrBusy}
                type="button"
                title="截图翻译 (Ctrl+Shift+R)"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <rect x="3" y="3" width="18" height="18" rx="2" />
                  <circle cx="8.5" cy="8.5" r="1.5" />
                  <path d="M21 15l-5-5L5 21" />
                </svg>
              </button>
              <button
                className={`sidebar__tool-btn ${isLiveActive ? "sidebar__tool-btn--live" : ""}`}
                onClick={startLiveTranslation}
                type="button"
                title="开始实时翻译"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z" />
                  <path d="M19 10v2a7 7 0 01-14 0v-2" />
                  <line x1="12" y1="19" x2="12" y2="23" />
                  <line x1="8" y1="23" x2="16" y2="23" />
                </svg>
              </button>
            </div>
          </div>

          {uiaError && (
            <div className="sidebar__uia-error">{uiaError}</div>
          )}

          <div className="sidebar__output-section">
            <TranslationOutput
              originalText={currentOriginal}
              translatedText={currentResult?.translatedText ?? ""}
              isTranslating={isTranslating}
              error={translateError}
              engineId={currentResult?.engineId}
              latencyMs={currentResult?.latencyMs}
              onRetry={currentOriginal ? handleTranslate : undefined}
            />
          </div>

          <div className="sidebar__drop-hint">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
              <polyline points="17 8 12 3 7 8" />
              <line x1="12" y1="3" x2="12" y2="15" />
            </svg>
            <span>拖入文本文件翻译</span>
          </div>
        </div>
      )}

      {/* 实时模式区 — 单一滚动容器 */}
      {isLiveMode && (
        <div className="sidebar__live-scroll">
          {/* 实时状态栏 */}
          <div className="sidebar__live-bar">
            {isLiveActive ? (
              <button
                className="sidebar__live-stop-btn"
                onClick={stopLiveTranslation}
                type="button"
              >
                <span className="sidebar__live-dot" />
                停止
              </button>
            ) : (
              <button
                className="sidebar__live-restart-btn"
                onClick={startLiveTranslation}
                type="button"
              >
                重新开始
              </button>
            )}

            {isLiveActive && (
              <div className="sidebar__live-indicator-bar">
                <span className="sidebar__live-indicator" />
                <span className="sidebar__live-label">识别中</span>
              </div>
            )}
          </div>

          {liveError && (
            <div className="sidebar__live-error">{liveError}</div>
          )}

          {/* 当前正在识别的句子 */}
          {currentTranscript && (
            <div className="sidebar__live-current">
              {currentTranscript}
            </div>
          )}

          {/* 句子列表 */}
          {sentences.length > 0 && (
            <div className="sidebar__live-sentences" ref={sentencesRef}>
              {sentences.map((item, i) => (
                <div key={`${item.timestamp}-${i}`} className="sidebar__live-sentence">
                  <div className="sidebar__live-sentence-source">{item.transcript}</div>
                  {item.translation && (
                    <div className="sidebar__live-sentence-target">{item.translation}</div>
                  )}
                </div>
              ))}
            </div>
          )}

          {/* 整合翻译 */}
          {!isLiveActive && sentences.length > 0 && (
            <div className="sidebar__consolidate-section">
              <button
                className="sidebar__consolidate-btn"
                onClick={consolidate}
                disabled={isConsolidating}
                type="button"
              >
                {isConsolidating ? "整合翻译中..." : consolidatedTranslation ? "重新整合翻译" : "整合翻译"}
              </button>

              {consolidatedTranslation && (
                <div className="sidebar__consolidate-result">
                  <div className="sidebar__consolidate-header">
                    <span className="sidebar__consolidate-label">整合译文</span>
                    <button
                      className="sidebar__consolidate-copy"
                      onClick={() => {
                        navigator.clipboard.writeText(consolidatedTranslation).catch(() => {
                          const ta = document.createElement("textarea");
                          ta.value = consolidatedTranslation;
                          document.body.appendChild(ta);
                          ta.select();
                          document.execCommand("copy");
                          document.body.removeChild(ta);
                        });
                      }}
                      title="复制整合译文"
                      type="button"
                    >
                      复制
                    </button>
                  </div>
                  <div className="sidebar__consolidate-text">{consolidatedTranslation}</div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
