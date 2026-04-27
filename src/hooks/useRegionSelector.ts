import { useEffect, useRef } from "react";
import { useTranslationStore } from "../stores/translationStore";
import { useUiStore } from "../stores/uiStore";
import {
  onRegionSelected,
  captureScreenRegion,
  ocrRecognize,
  showBubbleWindow,
  moveBubbleToCursor,
  type RegionSelectedEvent,
} from "../lib/tauri-bridge";
import { withTimeout } from "../lib/withTimeout";
import { friendlyMessage } from "../lib/errorMessages";

/** 简单日志封装 */
const log = {
  info: (msg: string) => console.info(`[RegionOCR] ${msg}`),
  error: (msg: string) => console.error(`[RegionOCR] ${msg}`),
};

/**
 * 区域选择 → OCR → 翻译 管道 hook
 *
 * 职责：监听 region-selected → 截图 → OCR → 委托翻译管线
 * 不复制翻译管线逻辑（store 更新、气泡显示、历史保存由 pipeline.translate 处理）
 *
 * @param onTranslate - 翻译回调（pipeline.translate），契约：
 *   - 显示气泡（showBubbleWindow）
 *   - 写 store（currentOriginal / currentResult / translateError）
 *   - 保存历史（saveHistory）
 *   - 返回 awaitable Promise（await 后上述操作已完成）
 */
export function useRegionSelector(
  onTranslate: (text: string) => Promise<void>,
) {
  const onTranslateRef = useRef(onTranslate);
  onTranslateRef.current = onTranslate;

  const busyRef = useRef(false);
  const setOcrProcessing = useUiStore((s) => s.setOcrProcessing);
  const clearCurrent = useTranslationStore((s) => s.clearCurrent);
  const setTranslateError = useTranslationStore((s) => s.setTranslateError);

  // zustand selectors 返回稳定引用，onTranslate 通过 ref 绕过依赖追踪
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;

    onRegionSelected(async (region: RegionSelectedEvent) => {
      if (cancelled) return;
      if (busyRef.current) {
        log.info("上一次截图翻译仍在进行，忽略本次选区");
        return;
      }

      busyRef.current = true;

      log.info(
        `收到选区: ${region.x},${region.y} ${region.width}x${region.height}`,
      );

      setOcrProcessing(true);

      try {
        // 1. 截图（物理坐标直传）
        // TS 侧兜底超时 = Rust 侧超时 + 2s 余量
        // 保证 Rust 带前缀的错误（如 [SCREENSHOT_TIMEOUT]）先于 TS 到达，
        // 使 friendlyMessage() 能精确匹配。此值不是 SLA 目标，勿与 Rust 超时机械同步。
        const base64Png = await withTimeout(
          captureScreenRegion(
            region.x,
            region.y,
            region.width,
            region.height,
          ),
          12_000,
          "截图",
        );
        log.info(`截图成功: ${base64Png.length} chars base64`);

        // 2. OCR（同理，兜底超时 = Rust 侧 30s + 2s 余量）
        const ocrResult = await withTimeout(
          ocrRecognize(base64Png),
          32_000,
          "OCR 识别",
        );
        log.info(
          `OCR 完成: ${ocrResult.engineId}, ${ocrResult.latencyMs}ms, ${ocrResult.blocks.length} blocks`,
        );

        // 3. 空结果 → 清空旧结果 + 错误气泡
        if (!ocrResult.fullText.trim()) {
          log.info("OCR 未识别到文字");
          clearCurrent();
          setTranslateError("未识别到文字");
          try {
            await showBubbleWindow();
            await moveBubbleToCursor();
          } catch (bubbleErr) {
            log.error(`显示气泡失败: ${bubbleErr}`);
          }
          return;
        }

        // 4. 委托翻译管线（契约：await 后 store 已更新、气泡已显示、历史已保存）
        await onTranslateRef.current(ocrResult.fullText);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log.error(`截图翻译失败: ${msg}`);
        clearCurrent();
        setTranslateError(friendlyMessage(msg));
        try {
          await showBubbleWindow();
          await moveBubbleToCursor();
        } catch (bubbleErr) {
          log.error(`显示气泡失败: ${bubbleErr}`);
        }
      } finally {
        busyRef.current = false;
        if (!cancelled) {
          try {
            setOcrProcessing(false);
          } catch {
            // store 已销毁的极端情况，忽略
          }
        }
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
  }, []);
}
