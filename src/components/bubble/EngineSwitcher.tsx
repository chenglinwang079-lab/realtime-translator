import { useCallback, useEffect, useRef, useState } from "react";
import { getEngines, setDefaultEngine } from "../../lib/tauri-bridge";
import { useSettingsStore } from "../../stores/settingsStore";

interface Engine {
  id: string;
  name: string;
  available: boolean;
}

interface EngineSwitcherProps {
  currentEngineId: string;
  onEngineSwitch: (engineId: string) => void;
}

export function EngineSwitcher({
  currentEngineId,
  onEngineSwitch,
}: EngineSwitcherProps) {
  const [engines, setEngines] = useState<Engine[]>([]);
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const setSettings = useSettingsStore((s) => s.setSettings);

  useEffect(() => {
    getEngines().then(setEngines).catch(() => {});
  }, []);

  // 点击外部关闭
  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  const handleSelect = useCallback(
    async (engineId: string) => {
      if (engineId === currentEngineId) {
        setOpen(false);
        return;
      }
      try {
        await setDefaultEngine(engineId);
        setSettings({ defaultEngine: engineId });
        onEngineSwitch(engineId);
      } catch {
        // ignore
      }
      setOpen(false);
    },
    [currentEngineId, setSettings, onEngineSwitch],
  );

  const currentName =
    engines.find((e) => e.id === currentEngineId)?.name ?? currentEngineId;

  return (
    <div className="engine-switcher" ref={containerRef}>
      <button
        className="engine-switcher__trigger"
        onClick={() => setOpen(!open)}
        title="切换翻译引擎"
        type="button"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 01-2.83 2.83l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
        </svg>
        <span className="engine-switcher__name">{currentName}</span>
      </button>

      {open && engines.length > 0 && (
        <div className="engine-switcher__dropdown">
          {engines.map((engine) => (
            <button
              key={engine.id}
              className={`engine-switcher__option ${
                engine.id === currentEngineId
                  ? "engine-switcher__option--active"
                  : ""
              } ${!engine.available ? "engine-switcher__option--disabled" : ""}`}
              onClick={() => engine.available && handleSelect(engine.id)}
              disabled={!engine.available}
              type="button"
            >
              <span className="engine-switcher__option-name">{engine.name}</span>
              {!engine.available && (
                <span className="engine-switcher__option-badge">未配置</span>
              )}
              {engine.id === currentEngineId && (
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
