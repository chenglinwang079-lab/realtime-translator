import { useCallback } from "react";

interface LanguageSettingsProps {
  defaultSourceLang: string;
  defaultTargetLang: string;
  onSourceLangChange: (lang: string) => void;
  onTargetLangChange: (lang: string) => void;
}

const LANGUAGES = [
  { code: "auto", name: "自动检测" },
  { code: "zh", name: "中文" },
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "ru", name: "Русский" },
];

export function LanguageSettings({
  defaultSourceLang,
  defaultTargetLang,
  onSourceLangChange,
  onTargetLangChange,
}: LanguageSettingsProps) {
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
    <div className="settings-section">
      <h3 className="settings-section__title">语言设置</h3>

      {/* 默认源语言 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <label className="settings-item__label" htmlFor="source-lang">
            默认源语言
          </label>
          <span className="settings-item__desc">
            自动检测或指定默认源语言
          </span>
        </div>
        <select
          id="source-lang"
          className="settings-item__select"
          value={defaultSourceLang}
          onChange={handleSourceChange}
        >
          {LANGUAGES.map((lang) => (
            <option key={lang.code} value={lang.code}>
              {lang.name}
            </option>
          ))}
        </select>
      </div>

      {/* 默认目标语言 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <label className="settings-item__label" htmlFor="target-lang">
            默认目标语言
          </label>
          <span className="settings-item__desc">翻译结果的目标语言</span>
        </div>
        <select
          id="target-lang"
          className="settings-item__select"
          value={defaultTargetLang}
          onChange={handleTargetChange}
        >
          {LANGUAGES.filter((l) => l.code !== "auto").map((lang) => (
            <option key={lang.code} value={lang.code}>
              {lang.name}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
}
