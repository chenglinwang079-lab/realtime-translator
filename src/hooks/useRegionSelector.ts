import { useEffect, useState } from "react";
import {
  onRegionSelected,
  captureScreenRegion,
  type RegionSelectedEvent,
} from "../lib/tauri-bridge";

/** 简单日志封装 */
const log = {
  info: (msg: string) => console.info(`[RegionSelector] ${msg}`),
  error: (msg: string) => console.error(`[RegionSelector] ${msg}`),
};

/**
 * 区域选择器 hook — 在主窗口（bubble）中监听选区完成事件
 *
 * 流程：
 * 1. 监听 `region-selected` 事件（Rust 转换后的全局物理坐标）
 * 2. 调用 `captureScreenRegion` 截图
 * 3. 截图 base64 存入 state（后续 OCR → 翻译由 2.2.8 串联）
 */
export function useRegionSelector() {
  const [screenshot, setScreenshot] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    onRegionSelected((region: RegionSelectedEvent) => {
      log.info(
        `收到选区: ${region.x},${region.y} ${region.width}x${region.height}`
      );

      // 截图（物理坐标直传）
      captureScreenRegion(region.x, region.y, region.width, region.height)
        .then((base64Png) => {
          setScreenshot(base64Png);
          log.info(`截图成功: ${base64Png.length} chars base64`);
          // TODO: 2.2.8 — 这里触发 OCR → 翻译 → 气泡显示
        })
        .catch((err) => {
          log.error(`截图失败: ${err}`);
        });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  return {
    /** 最近一次选区截图的 base64 PNG（可用于后续 OCR） */
    screenshot,
  };
}
