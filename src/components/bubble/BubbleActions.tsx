import { useCallback } from "react";

interface BubbleActionsProps {
  translatedText: string;
  onCopy: () => void;
  onPin: () => void;
  onClose: () => void;
  onOpenSidebar: () => void;
}

export function BubbleActions({
  translatedText,
  onCopy,
  onPin,
  onClose,
  onOpenSidebar,
}: BubbleActionsProps) {
  const handleCopy = useCallback(() => {
    if (translatedText) {
      navigator.clipboard.writeText(translatedText);
      onCopy();
    }
  }, [translatedText, onCopy]);

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
