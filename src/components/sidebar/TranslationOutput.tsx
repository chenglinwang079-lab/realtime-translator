import { useCallback, useRef } from "react";

interface TranslationOutputProps {
  translatedText: string;
  isTranslating: boolean;
  error?: string | null;
  engineId?: string;
  latencyMs?: number;
}

export function TranslationOutput({
  translatedText,
  isTranslating,
  error,
  engineId,
  latencyMs,
}: TranslationOutputProps) {
  const outputRef = useRef<HTMLDivElement>(null);

  const handleCopy = useCallback(() => {
    if (translatedText) {
      navigator.clipboard.writeText(translatedText);
    }
  }, [translatedText]);

  return (
    <div className="translation-output">
      <div className="translation-output__header">
        <span className="translation-output__label">译文</span>
        {translatedText && !isTranslating && (
          <button
            className="translation-output__copy"
            onClick={handleCopy}
            title="复制译文"
            type="button"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <rect x="9" y="9" width="13" height="13" rx="2" />
              <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
            </svg>
          </button>
        )}
      </div>

      <div
        ref={outputRef}
        className={`translation-output__content ${isTranslating ? "translation-output__content--loading" : ""}`}
      >
        {isTranslating ? (
          <div className="translation-output__loading">
            <span className="dot-pulse" />
            <span>翻译中...</span>
          </div>
        ) : error ? (
          <div className="translation-output__error">
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            <span>{error}</span>
          </div>
        ) : translatedText ? (
          <div className="translation-output__text">{translatedText}</div>
        ) : (
          <div className="translation-output__empty">
            翻译结果将显示在这里
          </div>
        )}
      </div>

      {engineId && latencyMs && !isTranslating && !error && (
        <div className="translation-output__meta">
          <span className="translation-output__engine">{engineId}</span>
          <span className="translation-output__latency">{latencyMs}ms</span>
        </div>
      )}
    </div>
  );
}
