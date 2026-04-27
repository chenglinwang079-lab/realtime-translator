import { useCallback, useState } from "react";

interface Engine {
  id: string;
  name: string;
  available: boolean;
}

interface EngineConfigProps {
  engines: Engine[];
  defaultEngine: string;
  title?: string;
  showDefaultSelector?: boolean;
  onDefaultEngineChange: (engineId: string) => void;
  onTestEngine: (engineId: string) => Promise<{ success: boolean; latencyMs: number }>;
  onSaveApiKey: (engineId: string, apiKey: string, extra?: string) => Promise<void>;
  onDeleteApiKey: (engineId: string) => Promise<void>;
}

export function EngineConfig({
  engines,
  defaultEngine,
  title = "翻译引擎",
  showDefaultSelector = true,
  onDefaultEngineChange,
  onTestEngine,
  onSaveApiKey,
  onDeleteApiKey,
}: EngineConfigProps) {
  const [testingEngine, setTestingEngine] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{
    engineId: string;
    success: boolean;
    latencyMs: number;
  } | null>(null);
  const [editingEngine, setEditingEngine] = useState<string | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [extraInput, setExtraInput] = useState("");

  const handleTest = useCallback(
    async (engineId: string) => {
      setTestingEngine(engineId);
      setTestResult(null);
      try {
        const result = await onTestEngine(engineId);
        setTestResult({ engineId, ...result });
      } catch {
        setTestResult({ engineId, success: false, latencyMs: 0 });
      } finally {
        setTestingEngine(null);
      }
    },
    [onTestEngine]
  );

  const handleSaveApiKey = useCallback(
    async (engineId: string) => {
      const key = apiKeyInput.trim();
      if (!key) return;

      if (engineId === "tencent-tmt" || engineId === "baidu-ocr") {
        const secretKey = extraInput.trim();
        if (!secretKey) return;
        await onSaveApiKey(engineId, key, secretKey);
      } else {
        await onSaveApiKey(engineId, key);
      }
      setEditingEngine(null);
      setApiKeyInput("");
      setExtraInput("");
    },
    [apiKeyInput, extraInput, onSaveApiKey]
  );

  const handleDeleteApiKey = useCallback(
    async (engineId: string) => {
      await onDeleteApiKey(engineId);
    },
    [onDeleteApiKey]
  );

  return (
    <div className="settings-section">
      <h3 className="settings-section__title">{title}</h3>

      {/* 默认引擎选择 */}
      {showDefaultSelector && (
        <div className="settings-item">
        <div className="settings-item__info">
          <label className="settings-item__label" htmlFor="default-engine">
            默认引擎
          </label>
          <span className="settings-item__desc">选择首选翻译引擎</span>
        </div>
        <select
          id="default-engine"
          className="settings-item__select"
          value={defaultEngine}
          onChange={(e) => onDefaultEngineChange(e.target.value)}
        >
          {engines.map((engine) => (
            <option key={engine.id} value={engine.id}>
              {engine.name}
            </option>
          ))}
        </select>
      </div>
      )}

      {/* 引擎列表 */}
      <div className="engine-list">
        {engines.map((engine) => (
          <div
            key={engine.id}
            className={`engine-card ${engine.available ? "" : "engine-card--unavailable"}`}
          >
            <div className="engine-card__header">
              <div className="engine-card__info">
                <span className="engine-card__name">{engine.name}</span>
                <span
                  className={`engine-card__status ${engine.available ? "engine-card__status--active" : "engine-card__status--inactive"}`}
                >
                  {engine.available ? "可用" : "未配置"}
                </span>
              </div>
              <div className="engine-card__actions">
                <button
                  className="engine-card__btn"
                  onClick={() => handleTest(engine.id)}
                  disabled={testingEngine === engine.id || !engine.available}
                  type="button"
                >
                  {testingEngine === engine.id ? "测试中..." : "测试"}
                </button>
                {engine.available ? (
                  <button
                    className="engine-card__btn engine-card__btn--danger"
                    onClick={() => handleDeleteApiKey(engine.id)}
                    type="button"
                  >
                    移除 Key
                  </button>
                ) : (
                  <button
                    className="engine-card__btn engine-card__btn--primary"
                    onClick={() => {
                      setEditingEngine(engine.id);
                      setApiKeyInput("");
                      setExtraInput("");
                    }}
                    type="button"
                  >
                    配置
                  </button>
                )}
              </div>
            </div>

            {/* 测试结果 */}
            {testResult && testResult.engineId === engine.id && (
              <div
                className={`engine-card__test-result ${testResult.success ? "engine-card__test-result--success" : "engine-card__test-result--error"}`}
              >
                {testResult.success
                  ? `✓ 测试成功，延迟 ${testResult.latencyMs}ms`
                  : "✗ 测试失败，请检查配置"}
              </div>
            )}

            {/* API Key 输入 */}
            {editingEngine === engine.id && (
              <div className="engine-card__api-key-form">
                <input
                  type="password"
                  className="engine-card__api-key-input"
                  placeholder={engine.id === "tencent-tmt" ? "Secret ID" : "输入 API Key"}
                  value={apiKeyInput}
                  onChange={(e) => setApiKeyInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      handleSaveApiKey(engine.id);
                    }
                  }}
                />
                {(engine.id === "tencent-tmt" || engine.id === "baidu-ocr") && (
                  <input
                    type="password"
                    className="engine-card__api-key-input"
                    placeholder="Secret Key"
                    value={extraInput}
                    onChange={(e) => setExtraInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        handleSaveApiKey(engine.id);
                      }
                    }}
                  />
                )}
                <div className="engine-card__api-key-actions">
                  <button
                    className="engine-card__btn engine-card__btn--primary"
                    onClick={() => handleSaveApiKey(engine.id)}
                    disabled={
                      !apiKeyInput.trim() ||
                      ((engine.id === "tencent-tmt" || engine.id === "baidu-ocr") && !extraInput.trim())
                    }
                    type="button"
                  >
                    保存
                  </button>
                  <button
                    className="engine-card__btn"
                    onClick={() => {
                      setEditingEngine(null);
                      setApiKeyInput("");
                      setExtraInput("");
                    }}
                    type="button"
                  >
                    取消
                  </button>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
