import { useEffect, useRef } from "react";
import {
  startUiaEvents,
  stopUiaEvents,
  startUiaPollingFallback,
  stopUiaPollingFallback,
  onUiaTextEvent,
  type UiaTextEvent,
} from "../lib/tauri-bridge";

/**
 * UIA 事件监听 hook (事件驱动 + 轮询 fallback)
 *
 * @param translate - 翻译回调（通过 ref 保持引用稳定）
 * @param enabled - 是否启用划词即译（由设置开关控制）
 * @param onError - 启动失败时回调，用于回滚开关状态
 *
 * enabled=true  → 启动事件监听 + 轮询 fallback + 订阅事件
 * enabled=false → 取消订阅 + 停止两个服务（成对关闭）
 */
export function useUiaEventListener(
  translate: (text: string) => void,
  enabled: boolean,
  onError?: (error: unknown) => void,
) {
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastTextRef = useRef("");
  const translateRef = useRef(translate);
  translateRef.current = translate;
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;

  useEffect(() => {
    if (!enabled) {
      // 成对停止两个服务
      stopUiaEvents().catch((e) => console.warn("[UIA] Failed to stop events:", e));
      stopUiaPollingFallback().catch((e) => console.warn("[UIA] Failed to stop polling:", e));
      return;
    }

    // 启动两个服务（幂等 — 后端 AtomicBool 守卫）
    // 任一失败则调用 onError 回滚开关状态
    Promise.all([startUiaEvents(), startUiaPollingFallback()]).catch(
      (e) => {
        console.error("[UIA] Failed to start services:", e);
        onErrorRef.current?.(e);
      },
    );

    // 订阅事件
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    onUiaTextEvent((event: UiaTextEvent) => {
      if (event.eventType !== "selection-changed" || !event.text) {
        return;
      }
      if (event.text === lastTextRef.current) {
        return;
      }
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      debounceRef.current = setTimeout(() => {
        lastTextRef.current = event.text;
        translateRef.current(event.text);
      }, 300);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    }).catch((e) => {
      console.error("[UIA] Failed to listen for text events:", e);
    });

    return () => {
      cancelled = true;
      unlisten?.();
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      // 成对停止两个服务
      stopUiaEvents().catch((e) => console.warn("[UIA] Failed to stop events:", e));
      stopUiaPollingFallback().catch((e) => console.warn("[UIA] Failed to stop polling:", e));
    };
  }, [enabled]);
}
