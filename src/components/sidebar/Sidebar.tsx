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

  // 统一 busy 语义：OCR 阶段或翻译阶段都算忙碌
  const screenOcrBusy = ocrProcessing || isTranslating;

  const [inputText, setInputText] = useState(currentOriginal);
  const [sourceLang, setSourceLang] = useState("auto");
  const [targetLang, setTargetLang] = useState("zh");

  // 拖拽调整宽度
  const sidebarRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(SIDEBAR_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = useState(false);
  const MIN_WIDTH = 300;
  const MAX_WIDTH = 600;

  // 侧边栏打开/关闭时调整窗口宽度（保持当前高度）
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

  // 同步外部状态到输入框
  useEffect(() => {
    if (currentOriginal && currentOriginal !== inputText) {
      setInputText(currentOriginal);
    }
  }, [currentOriginal]);

  // PoC 3: 监听 Ctrl+Shift+G 快捷键抓取的文本
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
  }, [
    inputText,
    setCurrentOriginal,
    setTranslating,
    setTranslateError,
    setCurrentResult,
  ]);

  const handleSwapLanguages = useCallback(() => {
    if (sourceLang === "auto") return;
    const newSource = targetLang;
    const newTarget = sourceLang;
    setSourceLang(newSource);
    setTargetLang(newTarget);

    // 如果有译文，交换原文和译文
    if (currentResult?.translatedText) {
      setInputText(currentResult.translatedText);
      setCurrentOriginal(currentResult.translatedText);
    }
  }, [
    sourceLang,
    targetLang,
    currentResult,
    setCurrentOriginal,
  ]);

  const handleClose = useCallback(() => {
    setSidebarVisible(false);
  }, [setSidebarVisible]);

  // 拖拽调整宽度
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

  // 文件拖入处理
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

  // PoC 3: 通过 UIA 抓取其他应用的选中文本
  const [uiaError, setUiaError] = useState("");
  const [isGrabbing, setIsGrabbing] = useState(false);
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

      // 只处理文本文件
      const textFile = files.find((f) =>
        f.type.startsWith("text/") ||
        f.name.endsWith(".txt") ||
        f.name.endsWith(".md") ||
        f.name.endsWith(".csv") ||
        f.name.endsWith(".json") ||
        f.name.endsWith(".log")
      );

      if (textFile) {
        // 限制文件大小 100KB
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
            // 自动触发翻译
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
      className={`sidebar ${isResizing ? "sidebar--resizing" : ""}`}
      style={{ width: `${width}px` }}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {/* 拖拽调整手柄 */}
      <div
        className="sidebar__resize-handle"
        onMouseDown={handleResizeStart}
      />

      {/* 侧边栏头部 */}
      <div className="sidebar__header">
        <h2 className="sidebar__title">翻译</h2>
        <button
          className="sidebar__close"
          onClick={handleClose}
          title="关闭侧边栏"
          type="button"
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      {/* 语言选择器 */}
      <LanguagePair
        sourceLang={sourceLang}
        targetLang={targetLang}
        onSourceLangChange={setSourceLang}
        onTargetLangChange={setTargetLang}
        onSwap={handleSwapLanguages}
      />

      {/* 输入区域 */}
      <TextInput
        value={inputText}
        onChange={setInputText}
        onTranslate={handleTranslate}
        disabled={isTranslating}
      />

      {/* 翻译按钮 */}
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

      {/* PoC 3: UIA 抓取按钮 */}
      <button
        className={`sidebar__grab-btn ${isGrabbing ? "sidebar__grab-btn--loading" : ""}`}
        onClick={handleGrabText}
        disabled={isGrabbing}
        type="button"
        title="通过 Windows UI Automation 抓取其他应用中选中的文本（快捷键: Ctrl+Shift+G）"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5" />
        </svg>
        {isGrabbing ? "抓取中..." : "抓取选中文本 (Ctrl+Shift+G)"}
      </button>
      {uiaError && (
        <div className="sidebar__uia-error">{uiaError}</div>
      )}

      {/* 截图翻译按钮 */}
      <button
        className={`sidebar__ocr-btn ${ocrProcessing ? "sidebar__ocr-btn--loading" : ""}`}
        onClick={() => showRegionSelector()}
        disabled={screenOcrBusy}
        type="button"
        title="框选屏幕区域进行 OCR 截图翻译（快捷键: Ctrl+Shift+R）"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <path d="M21 15l-5-5L5 21" />
        </svg>
        {ocrProcessing ? "识别中..." : isTranslating ? "翻译中..." : "截图翻译 (Ctrl+Shift+R)"}
      </button>

      {/* 翻译结果 */}
      <TranslationOutput
        translatedText={currentResult?.translatedText ?? ""}
        isTranslating={isTranslating}
        error={translateError}
        engineId={currentResult?.engineId}
        latencyMs={currentResult?.latencyMs}
        onRetry={currentOriginal ? handleTranslate : undefined}
      />

      {/* 文件拖入提示 */}
      <div className="sidebar__drop-hint">
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
          <polyline points="17 8 12 3 7 8" />
          <line x1="12" y1="3" x2="12" y2="15" />
        </svg>
        <span>拖入文本文件翻译</span>
      </div>
    </div>
  );
}
