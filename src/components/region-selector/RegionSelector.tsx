import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  submitRegionSelection,
  cancelRegionSelection,
} from "../../lib/tauri-bridge";
import "./region-selector.css";

interface Point {
  x: number;
  y: number;
}

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 将起点+终点转为标准矩形（x/y 为左上角，w/h 为正数） */
function toRect(start: Point, end: Point): Rect {
  const x = Math.min(start.x, end.x);
  const y = Math.min(start.y, end.y);
  const width = Math.abs(end.x - start.x);
  const height = Math.abs(end.y - start.y);
  return { x, y, width, height };
}

export function RegionSelector() {
  const [isDrawing, setIsDrawing] = useState(false);
  const [start, setStart] = useState<Point | null>(null);
  const [current, setCurrent] = useState<Point | null>(null);
  const submittedRef = useRef(false);
  const startRef = useRef<Point | null>(null);

  // 当前选区矩形
  const rect = start && current ? toRect(start, current) : null;

  // 确保窗口获焦（Esc 依赖焦点）
  useEffect(() => {
    getCurrentWindow()
      .setFocus()
      .catch((e) => console.warn("setFocus failed:", e));
  }, []);

  // Esc 取消
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelRegionSelection().catch((err) =>
          console.error("cancel failed:", err)
        );
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // mousemove/mouseup 绑定到 window 级别，防止鼠标移出窗口丢失事件
  useEffect(() => {
    if (!isDrawing) return;

    const handleMouseMove = (e: MouseEvent) => {
      setCurrent({ x: e.clientX, y: e.clientY });
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (submittedRef.current) return;
      setIsDrawing(false);

      const end = { x: e.clientX, y: e.clientY };
      const finalRect = startRef.current
        ? toRect(startRef.current, end)
        : null;

      // 最小选区检查（至少 5x5）
      if (!finalRect || finalRect.width < 5 || finalRect.height < 5) {
        setStart(null);
        setCurrent(null);
        startRef.current = null;
        return;
      }

      submittedRef.current = true;

      // 上报逻辑坐标给 Rust，Rust 侧做 scale_factor 转换
      submitRegionSelection(
        finalRect.x,
        finalRect.y,
        finalRect.width,
        finalRect.height
      ).catch((err) => {
        console.error("submit failed:", err);
      });
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDrawing]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // 仅左键
    if (e.button !== 0) return;
    submittedRef.current = false;
    const p = { x: e.clientX, y: e.clientY };
    startRef.current = p;
    setStart(p);
    setCurrent(p);
    setIsDrawing(true);
  }, []);

  // 禁用右键菜单
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
  }, []);

  const hasRect = rect && rect.width > 0 && rect.height > 0;

  return (
    <div
      className="region-selector"
      onMouseDown={handleMouseDown}
      onContextMenu={handleContextMenu}
    >
      {hasRect && (
        <>
          {/* 四块遮罩 */}
          <div
            className="region-selector__mask region-selector__mask--top"
            style={{ height: rect.y }}
          />
          <div
            className="region-selector__mask region-selector__mask--bottom"
            style={{ top: rect.y + rect.height }}
          />
          <div
            className="region-selector__mask region-selector__mask--left"
            style={{ top: rect.y, width: rect.x, height: rect.height }}
          />
          <div
            className="region-selector__mask region-selector__mask--right"
            style={{
              top: rect.y,
              left: rect.x + rect.width,
              height: rect.height,
            }}
          />

          {/* 选区边框 */}
          <div
            className="region-selector__rect"
            style={{
              left: rect.x,
              top: rect.y,
              width: rect.width,
              height: rect.height,
            }}
          />

          {/* 尺寸提示 */}
          <div
            className="region-selector__size"
            style={{
              left: rect.x + rect.width,
              top: rect.y + rect.height,
            }}
          >
            {Math.round(rect.width)} × {Math.round(rect.height)}
          </div>
        </>
      )}
    </div>
  );
}
