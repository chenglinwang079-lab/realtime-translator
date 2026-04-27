import { useCallback } from "react";

interface TranslationResultProps {
  originalText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
  isTranslating: boolean;
  error: string;
}

export function TranslationResult({
  originalText,
  translatedText,
  sourceLang,
  targetLang,
  engineId,
  latencyMs,
  isTranslating,
  error,
}: TranslationResultProps) {
  const copyTranslation = useCallback(() => {
    if (translatedText) {
      navigator.clipboard.writeText(translatedText);
    }
  }, [translatedText]);

  return (
    <div className="translation-result">
      <div className="translation-result__original">
        <div className="translation-result__label">
          原文
          {sourceLang && (
            <span className="translation-result__lang">{sourceLang}</span>
          )}
        </div>
        <div className="translation-result__text">{originalText}</div>
      </div>

      <div className="translation-result__divider" />

      <div className="translation-result__translated">
        <div className="translation-result__label">
          译文
          {targetLang && (
            <span className="translation-result__lang">{targetLang}</span>
          )}
        </div>

        {isTranslating && (
          <div className="translation-result__loading">
            <span className="dot" />
            <span className="dot" />
            <span className="dot" />
          </div>
        )}

        {error && <div className="translation-result__error">{error}</div>}

        {!isTranslating && !error && translatedText && (
          <div
            className="translation-result__text translation-result__text--translated"
            onClick={copyTranslation}
            title="点击复制译文"
          >
            {translatedText}
          </div>
        )}
      </div>

      {latencyMs > 0 && (
        <div className="translation-result__meta">
          <span className="translation-result__engine">{engineId}</span>
          <span className="translation-result__latency">{latencyMs}ms</span>
        </div>
      )}
    </div>
  );
}
