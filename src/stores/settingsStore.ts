import { create } from "zustand";
import type { AppSettings } from "../lib/tauri-bridge";
import { getSettings, updateSettings } from "../lib/tauri-bridge";

const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  defaultSourceLang: "auto",
  defaultTargetLang: "auto",
  defaultEngine: "tencent-tmt",
  autoStart: false,
  enableHistory: true,
  shortcut: "Ctrl+Shift+T",
  enableUiaAutoTranslate: true,
  uiaBlacklist: [],
};

interface SettingsState {
  settings: AppSettings;
  loaded: boolean;

  // Actions
  setSettings: (settings: Partial<AppSettings>) => void;
  loadSettings: () => Promise<void>;
  setLoaded: (loaded: boolean) => void;
  resetSettings: () => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  loaded: false,

  setSettings: (partial) => {
    const newSettings = { ...get().settings, ...partial };
    set({ settings: newSettings });
    // 同步到后端数据库
    updateSettings(partial).catch((e) =>
      console.error("保存设置失败:", e)
    );
  },

  loadSettings: async () => {
    try {
      const saved = await getSettings();
      set({ settings: { ...DEFAULT_SETTINGS, ...saved }, loaded: true });
    } catch (e) {
      console.error("加载设置失败:", e);
      set({ loaded: true });
    }
  },

  setLoaded: (loaded) => set({ loaded }),

  resetSettings: () => {
    set({ settings: DEFAULT_SETTINGS });
    updateSettings(DEFAULT_SETTINGS).catch((e) =>
      console.error("重置设置失败:", e)
    );
  },
}));
