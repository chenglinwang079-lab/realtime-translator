import { useCallback } from "react";
import { EngineSwitcher } from "./EngineSwitcher";

interface TranslationResultProps {
  originalText: string;
  translatedText: string;
  sourceLang: string;
  targetLang: string;
  engineId: string;
  latencyMs: number;
  isTranslating: boolean;
  error: string;
  onRetry?: () => void;
  onEngineSwitch?: (engineId: string) => void;
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
  onRetry,
  onEngineSwitch,
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

        {error && (
          <div className="translation-result__error">
            <span>{error}</span>
            {onRetry && (
              <button
                className="translation-result__retry"
                onClick={onRetry}
                type="button"
              >
                重试
              </button>
            )}
          </div>
        )}

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
          {onEngineSwitch ? (
            <EngineSwitcher
              currentEngineId={engineId}
              onEngineSwitch={onEngineSwitch}
            />
          ) : (
            <span className="translation-result__engine">{engineId}</span>
          )}
          <span className="translation-result__latency">{latencyMs}ms</span>
        </div>
      )}
    </div>
  );
}
