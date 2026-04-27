import { create } from "zustand";

export type BubbleState = "preview" | "interactive" | "pinned" | "dismissed";
export type SidebarTab = "translate" | "history" | "settings";

interface UiState {
  // Bubble
  bubbleState: BubbleState;

  // Sidebar
  sidebarVisible: boolean;
  sidebarTab: SidebarTab;

  // Settings panel
  settingsOpen: boolean;

  // OCR processing (截图翻译全流程 loading)
  ocrProcessing: boolean;

  // Actions
  setBubbleState: (state: BubbleState) => void;
  toggleSidebar: () => void;
  setSidebarVisible: (visible: boolean) => void;
  setSidebarTab: (tab: SidebarTab) => void;
  openSettings: () => void;
  closeSettings: () => void;
  toggleSettings: () => void;
  setOcrProcessing: (processing: boolean) => void;
}

export const useUiStore = create<UiState>((set) => ({
  bubbleState: "interactive",
  sidebarVisible: false,
  sidebarTab: "translate",
  settingsOpen: false,
  ocrProcessing: false,

  setBubbleState: (bubbleState) => set({ bubbleState }),

  toggleSidebar: () =>
    set((state) => ({ sidebarVisible: !state.sidebarVisible })),

  setSidebarVisible: (sidebarVisible) => set({ sidebarVisible }),

  setSidebarTab: (sidebarTab) => set({ sidebarTab }),

  openSettings: () => set({ settingsOpen: true }),

  closeSettings: () => set({ settingsOpen: false }),

  toggleSettings: () =>
    set((state) => ({ settingsOpen: !state.settingsOpen })),

  setOcrProcessing: (ocrProcessing) => set({ ocrProcessing }),
}));
