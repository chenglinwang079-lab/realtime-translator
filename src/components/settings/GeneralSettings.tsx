import { useCallback, useState } from "react";

interface GeneralSettingsProps {
  theme: "light" | "dark" | "system";
  autoStart: boolean;
  enableHistory: boolean;
  enableUiaAutoTranslate: boolean;
  uiaBlacklist: string[];
  onThemeChange: (theme: "light" | "dark" | "system") => void;
  onAutoStartChange: (enabled: boolean) => void;
  onEnableHistoryChange: (enabled: boolean) => void;
  onEnableUiaAutoTranslateChange: (enabled: boolean) => void;
  onAddBlacklistItem: (item: string) => void;
  onRemoveBlacklistItem: (index: number) => void;
}

export function GeneralSettings({
  theme,
  autoStart,
  enableHistory,
  enableUiaAutoTranslate,
  uiaBlacklist,
  onThemeChange,
  onAutoStartChange,
  onEnableHistoryChange,
  onEnableUiaAutoTranslateChange,
  onAddBlacklistItem,
  onRemoveBlacklistItem,
}: GeneralSettingsProps) {
  const [newItem, setNewItem] = useState("");

  const handleThemeChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      onThemeChange(e.target.value as "light" | "dark" | "system");
    },
    [onThemeChange]
  );

  const handleAddItem = useCallback(() => {
    const trimmed = newItem.trim();
    if (!trimmed) return;
    onAddBlacklistItem(trimmed);
    setNewItem("");
  }, [newItem, onAddBlacklistItem]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleAddItem();
      }
    },
    [handleAddItem]
  );

  return (
    <div className="settings-section">
      <h3 className="settings-section__title">通用</h3>

      {/* 主题 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <label className="settings-item__label" htmlFor="theme-select">
            主题
          </label>
          <span className="settings-item__desc">选择应用外观主题</span>
        </div>
        <select
          id="theme-select"
          className="settings-item__select"
          value={theme}
          onChange={handleThemeChange}
        >
          <option value="light">浅色</option>
          <option value="dark">深色</option>
          <option value="system">跟随系统</option>
        </select>
      </div>

      {/* 开机启动 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <span className="settings-item__label">开机自启动</span>
          <span className="settings-item__desc">系统启动时自动运行应用</span>
        </div>
        <label className="settings-item__toggle">
          <input
            type="checkbox"
            checked={autoStart}
            onChange={(e) => onAutoStartChange(e.target.checked)}
          />
          <span className="settings-item__toggle-slider" />
        </label>
      </div>

      {/* 翻译历史 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <span className="settings-item__label">保存翻译历史</span>
          <span className="settings-item__desc">记录翻译历史以便查看</span>
        </div>
        <label className="settings-item__toggle">
          <input
            type="checkbox"
            checked={enableHistory}
            onChange={(e) => onEnableHistoryChange(e.target.checked)}
          />
          <span className="settings-item__toggle-slider" />
        </label>
      </div>

      {/* 划词即译 */}
      <div className="settings-item">
        <div className="settings-item__info">
          <span className="settings-item__label">划词即译</span>
          <span className="settings-item__desc">
            选中文字自动翻译（仅 Windows）
          </span>
        </div>
        <label className="settings-item__toggle">
          <input
            type="checkbox"
            checked={enableUiaAutoTranslate}
            onChange={(e) => onEnableUiaAutoTranslateChange(e.target.checked)}
          />
          <span className="settings-item__toggle-slider" />
        </label>
      </div>

      {/* 应用黑名单 — 仅在划词即译开启时显示 */}
      {enableUiaAutoTranslate && (
        <>
          {/* VS Code / Electron 编辑器提示 */}
          <div className="uia-tips">
            <p className="uia-tips__item">
              VS Code / Cursor 等 Electron 编辑器需先按{" "}
              <kbd>Shift+Alt+F1</kbd> 启用屏幕阅读器模式，否则无法检测选中文本。
            </p>
            <p className="uia-tips__item">
              Chrome / Edge 等浏览器通过轮询自动支持，无需额外操作。
            </p>
          </div>

          <div className="settings-section settings-section--subsection">
            <h3 className="settings-section__title">应用黑名单</h3>
            <span className="settings-item__desc settings-item__desc--block">
              不翻译以下应用的文字（进程名或窗口标题关键词）
            </span>

            {/* 黑名单列表 */}
            {uiaBlacklist.length > 0 && (
              <div className="blacklist-items">
                {uiaBlacklist.map((item, index) => (
                  <div key={`${item}-${index}`} className="blacklist-item">
                    <span className="blacklist-item__name">{item}</span>
                    <button
                      className="engine-card__btn engine-card__btn--danger"
                      onClick={() => onRemoveBlacklistItem(index)}
                      type="button"
                    >
                      移除
                    </button>
                  </div>
                ))}
              </div>
            )}

            {/* 添加项 */}
            <div className="blacklist-add">
              <input
                className="blacklist-add__input"
                placeholder="输入进程名或关键词..."
                value={newItem}
                onChange={(e) => setNewItem(e.target.value)}
                onKeyDown={handleKeyDown}
              />
              <button
                className="engine-card__btn engine-card__btn--primary"
                onClick={handleAddItem}
                disabled={!newItem.trim()}
                type="button"
              >
                添加
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
