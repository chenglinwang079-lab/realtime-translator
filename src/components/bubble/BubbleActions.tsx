import { useCallback } from "react";

interface BubbleActionsProps {
  translatedText: string;
  onPin: () => void;
  onClose: () => void;
  onOpenSidebar: () => void;
  onOpenSettings: () => void;
}

export function BubbleActions({
  translatedText,
  onPin,
  onClose,
  onOpenSidebar,
  onOpenSettings,
}: BubbleActionsProps) {
  const handleCopy = useCallback(() => {
    if (translatedText) {
      navigator.clipboard.writeText(translatedText);
    }
  }, [translatedText]);

  return (
    <div className="bubble-actions">
      <button
        className="bubble-actions__btn"
        onClick={handleCopy}
        title="复制译文"
        disabled={!translatedText}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="9" y="9" width="13" height="13" rx="2" />
          <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
        </svg>
      </button>

      <button
        className="bubble-actions__btn"
        onClick={onPin}
        title="固定到侧边栏"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M12 17v5" />
          <path d="M9 11l-4 4h14l-4-4" />
          <path d="M12 3v8" />
          <circle cx="12" cy="3" r="2" />
        </svg>
      </button>

      <button
        className="bubble-actions__btn"
        onClick={onOpenSidebar}
        title="打开侧边栏"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <line x1="15" y1="3" x2="15" y2="21" />
        </svg>
      </button>

      <button
        className="bubble-actions__btn"
        onClick={onOpenSettings}
        title="设置"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
        </svg>
      </button>

      <div className="bubble-actions__spacer" />

      <button
        className="bubble-actions__btn bubble-actions__btn--close"
        onClick={onClose}
        title="关闭"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>
  );
}
