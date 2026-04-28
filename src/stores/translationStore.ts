import { create } from "zustand";
import type { TranslationResult } from "../lib/tauri-bridge";

export interface TranslationEntry {
  id: string;
  originalText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  timestamp: number;
  latencyMs: number;
}

interface TranslationState {
  // Current
  currentOriginal: string;
  currentResult: TranslationResult | null;
  isTranslating: boolean;
  translateError: string;

  // History
  history: TranslationEntry[];

  // Actions
  setCurrentOriginal: (text: string) => void;
  setCurrentResult: (result: TranslationResult | null) => void;
  setTranslating: (loading: boolean) => void;
  setTranslateError: (error: string) => void;
  clearCurrent: () => void;
  addToHistory: (entry: TranslationEntry) => void;
  loadHistory: (entries: TranslationEntry[]) => void;
  clearHistory: () => void;
}

export const useTranslationStore = create<TranslationState>((set) => ({
  currentOriginal: "",
  currentResult: null,
  isTranslating: false,
  translateError: "",
  history: [],

  setCurrentOriginal: (text) => set({ currentOriginal: text }),

  setCurrentResult: (result) => set({ currentResult: result }),

  setTranslating: (loading) => set({ isTranslating: loading }),

  setTranslateError: (error) => set({ translateError: error }),

  clearCurrent: () =>
    set({
      currentOriginal: "",
      currentResult: null,
      isTranslating: false,
      translateError: "",
    }),

  addToHistory: (entry) =>
    set((state) => ({
      history: [entry, ...state.history].slice(0, 200),
    })),

  loadHistory: (entries) => set({ history: entries }),

  clearHistory: () => set({ history: [] }),
}));
