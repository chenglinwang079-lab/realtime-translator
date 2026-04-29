import { useCallback, useState } from "react";

interface AsrConfigProps {
  appKey: string;
  accessKeyId: string;
  accessKeySecret: string;
  onAppKeyChange: (value: string) => void;
  onAccessKeyIdChange: (value: string) => void;
  onAccessKeySecretChange: (value: string) => void;
  onSave: () => Promise<void>;
  onTest?: () => Promise<number>;
}

export function AsrConfig({
  appKey,
  accessKeyId,
  accessKeySecret,
  onAppKeyChange,
  onAccessKeyIdChange,
  onAccessKeySecretChange,
  onSave,
  onTest,
}: AsrConfigProps) {
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const handleTest = useCallback(async () => {
    if (!onTest) return;
    setTesting(true);
    setTestResult(null);
    try {
      const latency = await onTest();
      setTestResult({ success: true, message: `连接成功 (${latency}ms)` });
    } catch (err) {
      setTestResult({ success: false, message: String(err) });
    } finally {
      setTesting(false);
    }
  }, [onTest]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    try {
      await onSave();
      setTestResult({ success: true, message: "保存成功" });
    } catch (err) {
      setTestResult({ success: false, message: String(err) });
    } finally {
      setSaving(false);
    }
  }, [onSave]);

  return (
    <div className="asr-config">
      <div className="settings-section">
        <h3 className="settings-section__title">阿里云实时语音识别</h3>
        <p className="asr-config__desc">
          配置阿里云实时语音识别 API，用于系统音频实时翻译功能。
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

        {testResult && (
          <div className={`asr-config__result ${testResult.success ? "asr-config__result--success" : "asr-config__result--error"}`}>
            {testResult.message}
          </div>
        )}

        <div className="asr-config__actions">
          {onTest && (
            <button
              className="asr-config__btn asr-config__btn--secondary"
              onClick={handleTest}
              disabled={testing || !appKey || !accessKeyId || !accessKeySecret}
              type="button"
            >
              {testing ? "测试中..." : "测试连接"}
            </button>
          )}
          <button
            className="asr-config__btn asr-config__btn--primary"
            onClick={handleSave}
            disabled={saving || !appKey || !accessKeyId || !accessKeySecret}
            type="button"
          >
            {saving ? "保存中..." : "保存配置"}
          </button>
        </div>
      </div>

      <div className="settings-section">
        <h3 className="settings-section__title">使用说明</h3>
        <div className="asr-config__help">
          <p>1. 登录阿里云控制台，开通"实时语音识别"服务</p>
          <p>2. 获取 App Key、Access Key ID 和 Access Key Secret</p>
          <p>3. 填入上方输入框并保存</p>
          <p>4. 在侧边栏点击"开始实时翻译"即可使用</p>
        </div>
      </div>
    </div>
  );
}
