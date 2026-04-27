import { useCallback, useEffect, useState } from "react";
import { GeneralSettings } from "./GeneralSettings";
import { EngineConfig } from "./EngineConfig";
import { ShortcutConfig } from "./ShortcutConfig";
import { LanguageSettings } from "./LanguageSettings";
import { useSettingsStore } from "../../stores/settingsStore";
import { useUiStore } from "../../stores/uiStore";
import {
  getEngines,
  setDefaultEngine,
  testEngine,
  saveApiKey,
  deleteApiKey,
  registerShortcut,
  unregisterShortcut,
  getOcrEngines,
  testOcrEngine,
} from "../../lib/tauri-bridge";
import "./settings.css";

type SettingsTab = "general" | "engine" | "shortcut" | "language";

interface Engine {
  id: string;
  name: string;
  available: boolean;
}

export function Settings() {
  const settingsOpen = useUiStore((s) => s.settingsOpen);
  const closeSettings = useUiStore((s) => s.closeSettings);

  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);

  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [engines, setEngines] = useState<Engine[]>([]);
  const [ocrEngines, setOcrEngines] = useState<Engine[]>([]);

  // 加载引擎列表
  useEffect(() => {
    if (settingsOpen) {
      getEngines()
        .then(setEngines)
        .catch((err) => console.error("Failed to load engines:", err));
      getOcrEngines()
        .then(setOcrEngines)
        .catch((err) => console.error("Failed to load OCR engines:", err));
    }
  }, [settingsOpen]);

  // ESC 关闭
  useEffect(() => {
    if (!settingsOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        closeSettings();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [settingsOpen, closeSettings]);

  // 通用设置变更
  const handleThemeChange = useCallback(
    (theme: "light" | "dark" | "system") => {
      setSettings({ theme });
    },
    [setSettings]
  );

  const handleAutoStartChange = useCallback(
    (autoStart: boolean) => {
      setSettings({ autoStart });
    },
    [setSettings]
  );

  const handleEnableHistoryChange = useCallback(
    (enableHistory: boolean) => {
      setSettings({ enableHistory });
    },
    [setSettings]
  );

  const handleEnableUiaAutoTranslateChange = useCallback(
    (enableUiaAutoTranslate: boolean) => {
      setSettings({ enableUiaAutoTranslate });
    },
    [setSettings]
  );

  // 黑名单变更
  const handleAddBlacklistItem = useCallback(
    (item: string) => {
      const list = settings.uiaBlacklist;
      const lower = item.toLowerCase();
      if (list.some((existing) => existing.toLowerCase() === lower)) return;
      setSettings({ uiaBlacklist: [...list, item] });
    },
    [setSettings, settings.uiaBlacklist]
  );

  const handleRemoveBlacklistItem = useCallback(
    (index: number) => {
      const list = settings.uiaBlacklist.filter((_, i) => i !== index);
      setSettings({ uiaBlacklist: list });
    },
    [setSettings, settings.uiaBlacklist]
  );

  // 引擎设置变更
  const handleDefaultEngineChange = useCallback(
    async (engineId: string) => {
      setSettings({ defaultEngine: engineId });
      await setDefaultEngine(engineId);
    },
    [setSettings]
  );

  const handleTestEngine = useCallback(async (engineId: string) => {
    return await testEngine(engineId);
  }, []);

  const handleTestOcrEngine = useCallback(async (engineId: string) => {
    const latencyMs = await testOcrEngine(engineId);
    return { success: true, latencyMs };
  }, []);

  const handleSaveApiKey = useCallback(
    async (engineId: string, apiKey: string, extra?: string) => {
      await saveApiKey(engineId, apiKey, extra);
      // 重新加载引擎列表
      const updatedEngines = await getEngines();
      setEngines(updatedEngines);
      const updatedOcr = await getOcrEngines();
      setOcrEngines(updatedOcr);
    },
    []
  );

  const handleDeleteApiKey = useCallback(async (engineId: string) => {
    await deleteApiKey(engineId);
    // 重新加载引擎列表
    const updatedEngines = await getEngines();
    setEngines(updatedEngines);
    const updatedOcr = await getOcrEngines();
    setOcrEngines(updatedOcr);
  }, []);

  // 快捷键设置变更
  const handleShortcutChange = useCallback(
    async (shortcut: string) => {
      const oldShortcut = settings.shortcut;
      // 先注册新快捷键，成功后再更新 UI 和 DB
      try {
        if (oldShortcut) {
          await unregisterShortcut(oldShortcut);
        }
        await registerShortcut(shortcut);
      } catch (err) {
        console.error("Failed to register shortcut:", err);
        // 注册失败，不更新 UI 和 DB
        return;
      }
      setSettings({ shortcut });
    },
    [setSettings, settings.shortcut]
  );

  // 语言设置变更
  const handleSourceLangChange = useCallback(
    (defaultSourceLang: string) => {
      setSettings({ defaultSourceLang });
    },
    [setSettings]
  );

  const handleTargetLangChange = useCallback(
    (defaultTargetLang: string) => {
      setSettings({ defaultTargetLang });
    },
    [setSettings]
  );

  if (!settingsOpen) {
    return null;
  }

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "general", label: "通用" },
    { id: "engine", label: "引擎" },
    { id: "shortcut", label: "快捷键" },
    { id: "language", label: "语言" },
  ];

  return (
    <div className="settings-overlay" onClick={closeSettings}>
      <div
        className="settings-modal"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 头部 */}
        <div className="settings-modal__header">
          <h2 className="settings-modal__title">设置</h2>
          <button
            className="settings-modal__close"
            onClick={closeSettings}
            title="关闭"
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

        {/* Tab 导航 */}
        <nav className="settings-modal__tabs">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              className={`settings-modal__tab ${activeTab === tab.id ? "settings-modal__tab--active" : ""}`}
              onClick={() => setActiveTab(tab.id)}
              type="button"
            >
              {tab.label}
            </button>
          ))}
        </nav>

        {/* 内容区域 */}
        <div className="settings-modal__content">
          {activeTab === "general" && (
            <GeneralSettings
              theme={settings.theme}
              autoStart={settings.autoStart}
              enableHistory={settings.enableHistory}
              enableUiaAutoTranslate={settings.enableUiaAutoTranslate}
              uiaBlacklist={settings.uiaBlacklist}
              onThemeChange={handleThemeChange}
              onAutoStartChange={handleAutoStartChange}
              onEnableHistoryChange={handleEnableHistoryChange}
              onEnableUiaAutoTranslateChange={handleEnableUiaAutoTranslateChange}
              onAddBlacklistItem={handleAddBlacklistItem}
              onRemoveBlacklistItem={handleRemoveBlacklistItem}
            />
          )}

          {activeTab === "engine" && (
            <>
              <EngineConfig
                engines={engines}
                defaultEngine={settings.defaultEngine}
                onDefaultEngineChange={handleDefaultEngineChange}
                onTestEngine={handleTestEngine}
                onSaveApiKey={handleSaveApiKey}
                onDeleteApiKey={handleDeleteApiKey}
              />
              <EngineConfig
                engines={ocrEngines}
                defaultEngine=""
                title="OCR 引擎"
                showDefaultSelector={false}
                onDefaultEngineChange={() => {}}
                onTestEngine={handleTestOcrEngine}
                onSaveApiKey={handleSaveApiKey}
                onDeleteApiKey={handleDeleteApiKey}
              />
            </>
          )}

          {activeTab === "shortcut" && (
            <ShortcutConfig
              currentShortcut={settings.shortcut}
              onShortcutChange={handleShortcutChange}
            />
          )}

          {activeTab === "language" && (
            <LanguageSettings
              defaultSourceLang={settings.defaultSourceLang}
              defaultTargetLang={settings.defaultTargetLang}
              onSourceLangChange={handleSourceLangChange}
              onTargetLangChange={handleTargetLangChange}
            />
          )}
        </div>
      </div>
    </div>
  );
}
