import { useCallback, useRef, useState } from "react";

interface TextInputProps {
  value: string;
  onChange: (value: string) => void;
  onTranslate: () => void;
  placeholder?: string;
  disabled?: boolean;
}

export function TextInput({
  value,
  onChange,
  onTranslate,
  placeholder = "输入或粘贴文本...",
  disabled = false,
}: TextInputProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [charCount, setCharCount] = useState(0);
  const MAX_CHARS = 5000;

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      if (newValue.length <= MAX_CHARS) {
        onChange(newValue);
        setCharCount(newValue.length);
      }
    },
    [onChange]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Ctrl+Enter 或 Cmd+Enter 触发翻译
      if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
        e.preventDefault();
        if (value.trim()) {
          onTranslate();
        }
      }
    },
    [value, onTranslate]
  );

  const handlePaste = useCallback(
    (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      // 限制粘贴长度
      const pastedText = e.clipboardData.getData("text");
      const currentText = value;
      const newText = currentText + pastedText;

      if (newText.length > MAX_CHARS) {
        e.preventDefault();
        const allowedLength = MAX_CHARS - currentText.length;
        const truncated = pastedText.slice(0, allowedLength);
        const textarea = textareaRef.current;
        if (textarea) {
          const start = textarea.selectionStart;
          const end = textarea.selectionEnd;
          const newValue =
            currentText.slice(0, start) + truncated + currentText.slice(end);
          onChange(newValue);
          setCharCount(newValue.length);

          // 恢复光标位置
          requestAnimationFrame(() => {
            textarea.selectionStart = start + truncated.length;
            textarea.selectionEnd = start + truncated.length;
          });
        }
      }
    },
    [value, onChange]
  );

  const handleClear = useCallback(() => {
    onChange("");
    setCharCount(0);
    textareaRef.current?.focus();
  }, [onChange]);

  return (
    <div className="text-input">
      <div className="text-input__header">
        <span className="text-input__label">原文</span>
        <span
          className={`text-input__count ${charCount > MAX_CHARS * 0.9 ? "text-input__count--warning" : ""}`}
        >
          {charCount}/{MAX_CHARS}
        </span>
      </div>

      <div className="text-input__container">
        <textarea
          ref={textareaRef}
          className="text-input__textarea"
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onPaste={handlePaste}
          placeholder={placeholder}
          disabled={disabled}
          rows={6}
          spellCheck={false}
        />

        {value && (
          <button
            className="text-input__clear"
            onClick={handleClear}
            title="清空"
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
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        )}
      </div>

      <div className="text-input__footer">
        <span className="text-input__hint">Ctrl+Enter 翻译</span>
      </div>
    </div>
  );
}
