import { useCallback, useState } from "react";

interface AsrConfigProps {
  // DashScope（新方案）
  dashscopeApiKey: string;
  onDashscopeApiKeyChange: (value: string) => void;
  onSaveDashScope: () => Promise<void>;
  // 旧 NLS 配置（保留）
  appKey: string;
  accessKeyId: string;
  accessKeySecret: string;
  onAppKeyChange: (value: string) => void;
  onAccessKeyIdChange: (value: string) => void;
  onAccessKeySecretChange: (value: string) => void;
  onSaveNls: () => Promise<void>;
}

export function AsrConfig({
  dashscopeApiKey,
  onDashscopeApiKeyChange,
  onSaveDashScope,
  appKey,
  accessKeyId,
  accessKeySecret,
  onAppKeyChange,
  onAccessKeyIdChange,
  onAccessKeySecretChange,
  onSaveNls,
}: AsrConfigProps) {
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const handleSaveDashScope = useCallback(async () => {
    setSaving(true);
    setTestResult(null);
    try {
      await onSaveDashScope();
      setTestResult({ success: true, message: "DashScope 配置已保存" });
    } catch (err) {
      setTestResult({ success: false, message: String(err) });
    } finally {
      setSaving(false);
    }
  }, [onSaveDashScope]);

  const handleSaveNls = useCallback(async () => {
    setSaving(true);
    setTestResult(null);
    try {
      await onSaveNls();
      setTestResult({ success: true, message: "NLS 配置已保存" });
    } catch (err) {
      setTestResult({ success: false, message: String(err) });
    } finally {
      setSaving(false);
    }
  }, [onSaveNls]);

  return (
    <div className="asr-config">
      {/* DashScope 配置（推荐） */}
      <div className="settings-section">
        <h3 className="settings-section__title">DashScope 实时语音识别（推荐）</h3>
        <p className="asr-config__desc">
          使用阿里云 DashScope FunASR Paraformer 引擎，识别质量更好，支持中英文。
        </p>

        <div className="asr-config__form">
          <div className="asr-config__field">
            <label className="asr-config__label">API Key</label>
            <input
              type="password"
              className="asr-config__input"
              value={dashscopeApiKey}
              onChange={(e) => onDashscopeApiKeyChange(e.target.value)}
              placeholder="sk-xxxxxxxxxxxxxxxxxxxxxxxx"
            />
          </div>
        </div>

        {testResult && (
          <div className={`asr-config__result ${testResult.success ? "asr-config__result--success" : "asr-config__result--error"}`}>
            {testResult.message}
          </div>
        )}

        <div className="asr-config__actions">
          <button
            className="asr-config__btn asr-config__btn--primary"
            onClick={handleSaveDashScope}
            disabled={saving || !dashscopeApiKey}
            type="button"
          >
            {saving ? "保存中..." : "保存配置"}
          </button>
        </div>
      </div>

      {/* 使用说明 */}
      <div className="settings-section">
        <h3 className="settings-section__title">如何获取 DashScope API Key</h3>
        <div className="asr-config__help">
          <p>1. 访问 <a href="https://dashscope.console.aliyun.com/" target="_blank" rel="noopener noreferrer">DashScope 控制台</a></p>
          <p>2. 登录阿里云账号</p>
          <p>3. 在「API-KEY 管理」中创建或复制 API Key</p>
          <p>4. 粘贴到上方输入框并保存</p>
          <p>5. 在侧边栏点击"开始实时翻译"即可使用</p>
        </div>
      </div>

      {/* 旧版 NLS 配置（折叠） */}
      <details className="settings-section asr-config__legacy">
        <summary className="settings-section__title">旧版 NLS 配置（不推荐）</summary>
        <p className="asr-config__desc">
          旧版阿里云 NLS 实时语音识别，识别质量较差，仅作备用。
        </p>

        <div className="asr-config__form">
          <div className="asr-config__field">
            <label className="asr-config__label">App Key</label>
            <input
              type="text"
              className="asr-config__input"
              value={appKey}
              onChange={(e) => onAppKeyChange(e.target.value)}
              placeholder="阿里云 App Key"
            />
          </div>

          <div className="asr-config__field">
            <label className="asr-config__label">Access Key ID</label>
            <input
              type="text"
              className="asr-config__input"
              value={accessKeyId}
              onChange={(e) => onAccessKeyIdChange(e.target.value)}
              placeholder="阿里云 Access Key ID"
            />
          </div>

          <div className="asr-config__field">
            <label className="asr-config__label">Access Key Secret</label>
            <input
              type="password"
              className="asr-config__input"
              value={accessKeySecret}
              onChange={(e) => onAccessKeySecretChange(e.target.value)}
              placeholder="阿里云 Access Key Secret"
            />
          </div>
        </div>

        <div className="asr-config__actions">
          <button
            className="asr-config__btn asr-config__btn--secondary"
            onClick={handleSaveNls}
            disabled={saving || !appKey || !accessKeyId || !accessKeySecret}
            type="button"
          >
            保存 NLS 配置
          </button>
        </div>
      </details>
    </div>
  );
}
