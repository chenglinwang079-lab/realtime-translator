import { useCallback } from "react";

export interface Language {
  code: string;
  name: string;
  nativeName: string;
}

const LANGUAGES: Language[] = [
  { code: "zh", name: "Chinese", nativeName: "中文" },
  { code: "en", name: "English", nativeName: "English" },
  { code: "ja", name: "Japanese", nativeName: "日本語" },
  { code: "ko", name: "Korean", nativeName: "한국어" },
  { code: "fr", name: "French", nativeName: "Français" },
  { code: "de", name: "German", nativeName: "Deutsch" },
  { code: "es", name: "Spanish", nativeName: "Español" },
  { code: "ru", name: "Russian", nativeName: "Русский" },
];

interface LanguagePairProps {
  sourceLang: string;
  targetLang: string;
  onSourceLangChange: (lang: string) => void;
  onTargetLangChange: (lang: string) => void;
  onSwap: () => void;
  autoDetect?: boolean;
}

export function LanguagePair({
  sourceLang,
  targetLang,
  onSourceLangChange,
  onTargetLangChange,
  onSwap,
  autoDetect = true,
}: LanguagePairProps) {
  const handleSourceChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      onSourceLangChange(e.target.value);
    },
    [onSourceLangChange]
  );

  const handleTargetChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      onTargetLangChange(e.target.value);
    },
    [onTargetLangChange]
  );

  return (
    <div className="language-pair">
      <div className="language-pair__select-wrapper">
        <select
          className="language-pair__select"
          value={sourceLang}
          onChange={handleSourceChange}
        >
          {autoDetect && <option value="auto">自动检测</option>}
          {LANGUAGES.map((lang) => (
            <option key={lang.code} value={lang.code}>
              {lang.nativeName}
            </option>
          ))}
        </select>
      </div>

      <button
        className="language-pair__swap"
        onClick={onSwap}
        title="交换语言"
        type="button"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M7 16l-4-4 4-4" />
          <path d="M3 12h18" />
          <path d="M17 8l4 4-4 4" />
        </svg>
      </button>

      <div className="language-pair__select-wrapper">
        <select
          className="language-pair__select"
          value={targetLang}
          onChange={handleTargetChange}
        >
          {LANGUAGES.map((lang) => (
            <option key={lang.code} value={lang.code}>
              {lang.nativeName}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
