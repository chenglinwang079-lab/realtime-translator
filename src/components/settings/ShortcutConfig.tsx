import { useCallback, useEffect, useRef, useState } from "react";

interface ShortcutConfigProps {
  currentShortcut: string;
  defaultShortcut?: string;
  onShortcutChange: (shortcut: string) => void;
}

export function ShortcutConfig({
  currentShortcut,
  defaultShortcut = "Ctrl+Shift+T",
  onShortcutChange,
}: ShortcutConfigProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [recordedKeys, setRecordedKeys] = useState<string[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);

  const handleStartRecording = useCallback(() => {
    setIsRecording(true);
    setRecordedKeys([]);
  }, []);

  const handleCancelRecording = useCallback(() => {
    setIsRecording(false);
    setRecordedKeys([]);
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!isRecording) return;

      e.preventDefault();
      e.stopPropagation();

      const key = e.key;

      // 忽略单独的修饰键
      if (["Control", "Shift", "Alt", "Meta"].includes(key)) {
        return;
      }

      // 构建快捷键字符串
      const parts: string[] = [];
      if (e.ctrlKey) parts.push("Ctrl");
      if (e.altKey) parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");
      if (e.metaKey) parts.push("Super");

      // 必须包含至少一个修饰键
      if (parts.length === 0) {
        return;
      }

      // 格式化键名
      const keyName = formatKeyName(key);
      parts.push(keyName);

      const shortcut = parts.join("+");
      setRecordedKeys(parts);
      onShortcutChange(shortcut);
      setIsRecording(false);
    },
    [isRecording, onShortcutChange]
  );

  useEffect(() => {
    if (isRecording) {
      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
    }
  }, [isRecording, handleKeyDown]);

  // 点击外部取消录制
  useEffect(() => {
    if (!isRecording) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setIsRecording(false);
        setRecordedKeys([]);
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    return () => window.removeEventListener("mousedown", handleClickOutside);
  }, [isRecording]);

  const displayKeys = isRecording
    ? recordedKeys
    : currentShortcut.split("+");

  return (
    <div className="settings-section">
      <h3 className="settings-section__title">快捷键</h3>

      <div className="settings-item">
        <div className="settings-item__info">
          <span className="settings-item__label">翻译快捷键</span>
          <span className="settings-item__desc">
            按下快捷键翻译剪贴板内容
          </span>
        </div>

        <div ref={containerRef} className="shortcut-config">
          <div
            className={`shortcut-config__display ${isRecording ? "shortcut-config__display--recording" : ""}`}
          >
            {isRecording ? (
              <span className="shortcut-config__hint">请按下快捷键...</span>
            ) : (
              displayKeys.map((key, i) => (
                <span key={i}>
                  <kbd className="shortcut-config__key">{key}</kbd>
                  {i < displayKeys.length - 1 && (
                    <span className="shortcut-config__separator">+</span>
                  )}
                </span>
              ))
            )}
          </div>

          {isRecording ? (
            <button
              className="shortcut-config__btn"
              onClick={handleCancelRecording}
              type="button"
            >
              取消
            </button>
          ) : (
            <div className="shortcut-config__actions">
              <button
                className="shortcut-config__btn"
                onClick={handleStartRecording}
                type="button"
              >
                更改
              </button>
              {currentShortcut !== defaultShortcut && (
                <button
                  className="shortcut-config__btn shortcut-config__btn--reset"
                  onClick={() => onShortcutChange(defaultShortcut)}
                  type="button"
                >
                  重置
                </button>
              )}
            </div>
          )}
        </div>
      </div>

      <div className="shortcut-config__tips">
        <p className="shortcut-config__tip">
          提示：快捷键必须包含至少一个修饰键（Ctrl、Alt、Shift 或 Super）
        </p>
      </div>
    </div>
  );
}

function formatKeyName(key: string): string {
  const keyMap: Record<string, string> = {
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
    Enter: "↵",
    Backspace: "⌫",
    Delete: "Del",
    Escape: "Esc",
    " ": "Space",
  };

  if (keyMap[key]) {
    return keyMap[key];
  }

  if (key.length === 1) {
    return key.toUpperCase();
  }

  return key;
}
