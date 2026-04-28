import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// === Types ===

export interface TranslationResult {
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
}

export interface ClipboardChangedEvent {
  text: string;
  source: string;
}

export interface CursorPosition {
  x: number;
  y: number;
}

export interface EngineInfo {
  id: string;
  name: string;
  available: boolean;
}

export interface AppSettings {
  theme: "light" | "dark" | "system";
  defaultSourceLang: string;
  defaultTargetLang: string;
  defaultEngine: string;
  autoStart: boolean;
  enableHistory: boolean;
  shortcut: string;
  enableUiaAutoTranslate: boolean;
  uiaBlacklist: string[];
}

export interface HistoryEntry {
  id: string;
  originalText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
  timestamp: string;
}

export interface TextSelection {
  text: string;
  appName: string;
  windowClass: string;
  windowTitle: string;
}

export interface UiaTextCapturedEvent {
  text: string | null;
  appName?: string;
  windowClass?: string;
  windowTitle?: string;
  error?: string;
}

// === Translation ===

export async function translate(text: string): Promise<TranslationResult> {
  return invoke<TranslationResult>("translate", { text });
}

export async function getEngines(): Promise<EngineInfo[]> {
  return invoke<EngineInfo[]>("get_engines");
}

export async function setDefaultEngine(engineId: string): Promise<void> {
  return invoke("set_default_engine", { engineId });
}

export async function testEngine(
  engineId: string
): Promise<{ success: boolean; latencyMs: number }> {
  return invoke("test_engine", { engineId });
}

export async function clearCache(): Promise<void> {
  return invoke("clear_cache");
}

// === Clipboard ===

export async function startClipboardWatch(): Promise<void> {
  return invoke("start_clipboard_watch");
}

export async function stopClipboardWatch(): Promise<void> {
  return invoke("stop_clipboard_watch");
}

export async function toggleClipboardWatch(): Promise<boolean> {
  return invoke("toggle_clipboard_watch");
}

export async function getWatchState(): Promise<boolean> {
  return invoke("get_watch_state");
}

export function onWatchStateChanged(
  callback: (watching: boolean) => void
): Promise<UnlistenFn> {
  return listen<boolean>("watch-state-changed", (e) => callback(e.payload));
}

export function onClipboardChanged(
  callback: (event: ClipboardChangedEvent) => void
): Promise<UnlistenFn> {
  return listen<ClipboardChangedEvent>("clipboard-changed", (e) =>
    callback(e.payload)
  );
}

// === Window ===

export async function getCursorPosition(): Promise<CursorPosition> {
  return invoke<CursorPosition>("get_cursor_position");
}

export async function moveBubbleToCursor(): Promise<void> {
  return invoke("move_bubble_to_cursor");
}

export async function moveBubbleFollow(dx: number, dy: number): Promise<void> {
  return invoke("move_bubble_follow", { dx, dy });
}

export async function showBubbleWindow(): Promise<void> {
  return invoke("show_bubble_window");
}

export async function hideBubbleWindow(): Promise<void> {
  return invoke("hide_bubble_window");
}

export async function setWindowState(
  state: "preview" | "interactive" | "pinned" | "dismissed"
): Promise<void> {
  return invoke("set_window_state", { state });
}

export async function setWindowSize(width: number, height: number): Promise<void> {
  return invoke("set_window_size", { width, height });
}

// === Settings ===

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: Partial<AppSettings>): Promise<void> {
  return invoke("update_settings", { settings });
}

export async function saveApiKey(engineId: string, apiKey: string, extra?: string): Promise<void> {
  return invoke("save_api_key", { engineId, apiKey, extra: extra ?? null });
}

export async function deleteApiKey(engineId: string): Promise<void> {
  return invoke("delete_api_key", { engineId });
}

// === History ===

export async function saveHistory(entry: HistoryEntry): Promise<void> {
  return invoke("save_history", { entry });
}

export async function getHistory(limit?: number): Promise<HistoryEntry[]> {
  return invoke("get_history", { limit: limit ?? null });
}

export async function clearHistory(): Promise<void> {
  return invoke("clear_history");
}

// === Shortcuts ===

export async function registerShortcut(shortcut: string): Promise<void> {
  return invoke("register_shortcut", { shortcut });
}

export async function unregisterShortcut(shortcut: string): Promise<void> {
  return invoke("unregister_shortcut", { shortcut });
}

export function onShortcutTriggered(
  callback: () => void
): Promise<UnlistenFn> {
  return listen("shortcut-triggered", () => callback());
}

// === Events ===

export function onOpenSettings(
  callback: () => void
): Promise<UnlistenFn> {
  return listen("open-settings", () => callback());
}

// === Screenshot ===

export async function captureScreen(): Promise<string> {
  // 返回 base64 PNG
  return invoke<string>("capture_screen");
}

export async function captureScreenRegion(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<string> {
  // x/y/width/height 为全局物理坐标，返回 base64 PNG
  return invoke<string>("capture_screen_region", { x, y, width, height });
}

// === Region Selector ===

export async function showRegionSelector(): Promise<void> {
  return invoke("show_region_selector");
}

export async function submitRegionSelection(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<void> {
  // x/y/width/height 是窗口内逻辑坐标（CSS 像素）
  // Rust 侧会乘以 scale_factor 转为物理坐标
  return invoke("submit_region_selection", { x, y, width, height });
}

export async function cancelRegionSelection(): Promise<void> {
  return invoke("cancel_region_selection");
}

// region-selected 事件中的坐标已经是全局物理坐标（Rust 转换后的）
export interface RegionSelectedEvent {
  x: number; // 全局物理 X
  y: number; // 全局物理 Y
  width: number; // 物理宽度
  height: number; // 物理高度
}

export function onRegionSelected(
  callback: (region: RegionSelectedEvent) => void
): Promise<UnlistenFn> {
  return listen<RegionSelectedEvent>("region-selected", (e) =>
    callback(e.payload)
  );
}

// === Accessibility (PoC 3) ===

export async function getSelectedText(): Promise<TextSelection | null> {
  return invoke<TextSelection | null>("get_selected_text");
}

export async function getFocusedApp(): Promise<string> {
  return invoke<string>("get_focused_app");
}

export function onUiaTextCaptured(
  callback: (event: UiaTextCapturedEvent) => void
): Promise<UnlistenFn> {
  return listen<UiaTextCapturedEvent>("uia-text-captured", (e) =>
    callback(e.payload)
  );
}

// === UIA Event Listener (auto-translate on selection) ===

export interface UiaTextEvent {
  text: string;
  appName: string;
  windowTitle: string;
  eventType: "focus-changed" | "selection-changed";
  source: "event" | "polling";
}

export async function startUiaEvents(): Promise<void> {
  return invoke("start_uia_events");
}

export async function stopUiaEvents(): Promise<void> {
  return invoke("stop_uia_events");
}

export async function getUiaEventsState(): Promise<boolean> {
  return invoke("get_uia_events_state");
}

export function onUiaTextEvent(
  callback: (event: UiaTextEvent) => void
): Promise<UnlistenFn> {
  return listen<UiaTextEvent>("uia-text-event", (e) => callback(e.payload));
}

// === UIA Polling Fallback ===

export async function startUiaPollingFallback(): Promise<void> {
  return invoke("start_uia_polling_fallback");
}

export async function stopUiaPollingFallback(): Promise<void> {
  return invoke("stop_uia_polling_fallback");
}

export async function getUiaPollingState(): Promise<boolean> {
  return invoke("get_uia_polling_state");
}

// === OCR ===

import type { OcrResult } from '../types/ocr';
export type { OcrResult, OcrTextBlock, OcrLevel } from '../types/ocr';

export async function ocrRecognize(imageBase64: string): Promise<OcrResult> {
  return invoke<OcrResult>('ocr_recognize', { imageBase64 });
}

export async function getOcrEngines(): Promise<EngineInfo[]> {
  return invoke<EngineInfo[]>('get_ocr_engines');
}

export async function testOcrEngine(engineId: string): Promise<number> {
  return invoke<number>('test_ocr_engine', { engineId });
}

export async function uninstallApp(): Promise<void> {
  return invoke('uninstall_app');
}
