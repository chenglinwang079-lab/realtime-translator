import { useCallback, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FloatingBubble } from "./components/bubble/FloatingBubble";
import { Sidebar } from "./components/sidebar/Sidebar";
import { Settings } from "./components/settings/Settings";
import { RegionSelector } from "./components/region-selector/RegionSelector";
import { useTranslationPipeline } from "./hooks/useTranslationPipeline";
import { useUiaEventListener } from "./hooks/useUiaEventListener";
import { useRegionSelector } from "./hooks/useRegionSelector";
import { useSettingsStore } from "./stores/settingsStore";
import { useTranslationStore } from "./stores/translationStore";
import { useUiStore } from "./stores/uiStore";
import { onOpenSettings, translate } from "./lib/tauri-bridge";
import "./App.css";

// 窗口路由：selector 窗口只渲染选区组件
const windowLabel = getCurrentWindow().label;

function App() {
  // selector 窗口：透明背景 + 只渲染 RegionSelector
  if (windowLabel === "selector") {
    document.documentElement.classList.add("transparent-window");
    return <RegionSelector />;
  }

  // 主窗口（bubble）：完整 UI
  return <BubbleApp />;
}

function BubbleApp() {
  // 初始化翻译管道（剪贴板监听 → 翻译 → 气泡显示）
  const pipeline = useTranslationPipeline();

  // UIA 事件监听（选中文字 → 自动翻译）
  const enableUia = useSettingsStore(
    (s) => s.settings.enableUiaAutoTranslate,
  );
  const loaded = useSettingsStore((s) => s.loaded);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const handleUiaError = useCallback(() => {
    setSettings({ enableUiaAutoTranslate: false });
  }, [setSettings]);
  useUiaEventListener(pipeline.translate, loaded && enableUia, handleUiaError);

  // 区域选择器（OCR 截图 → 翻译 → 气泡显示）
  useRegionSelector(pipeline.translate);

  // 重试翻译（OCR 失败时无原文，不显示重试按钮）
  const currentOriginal = useTranslationStore((s) => s.currentOriginal);
  const handleRetry = useCallback(() => {
    if (currentOriginal) {
      pipeline.translate(currentOriginal);
    }
  }, [pipeline.translate, currentOriginal]);

  const loadSettings = useSettingsStore((s) => s.loadSettings);
  const openSettings = useUiStore((s) => s.openSettings);
  const setSidebarVisible = useUiStore((s) => s.setSidebarVisible);

  // 文件拖入翻译
  const setCurrentOriginal = useTranslationStore((s) => s.setCurrentOriginal);
  const setCurrentResult = useTranslationStore((s) => s.setCurrentResult);
  const setTranslating = useTranslationStore((s) => s.setTranslating);
  const setTranslateError = useTranslationStore((s) => s.setTranslateError);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
  }, []);

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
          f.name.endsWith(".log"),
      );
      if (!textFile) return;
      if (textFile.size > 100 * 1024) {
        setTranslateError("文件过大，请拖入 100KB 以内的文本文件");
        setSidebarVisible(true);
        return;
      }
      const reader = new FileReader();
      reader.onload = async (event) => {
        const text = (event.target?.result as string)?.trim();
        if (!text) return;
        setCurrentOriginal(text);
        setSidebarVisible(true);
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
      };
      reader.readAsText(textFile);
    },
    [setCurrentOriginal, setCurrentResult, setTranslating, setTranslateError, setSidebarVisible],
  );

  // 启动时加载设置
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // 监听托盘"打开设置"事件
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onOpenSettings(() => {
      openSettings();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [openSettings]);

  // 启动时聚焦窗口
  useEffect(() => {
    getCurrentWindow().setFocus();
  }, []);

  return (
    <div className="app" onDragOver={handleDragOver} onDrop={handleDrop}>
      <FloatingBubble onRetry={handleRetry} />
      <Sidebar />
      <Settings />
    </div>
  );
}

export default App;
