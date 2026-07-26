import { useConfigStore } from '../../stores/configStore';
import { useTranslateStore } from '../../stores/translateStore';
import Card from '../../components/Card';
import { LANGUAGES } from '../../types';

export default function BasicSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const syncClipboardMonitorFromConfig = useTranslateStore((s) => s.syncClipboardMonitorFromConfig);

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-xl font-semibold text-text-primary">基础设置</h1>
        <p className="text-xs text-text-secondary mt-1">配置默认语言和基本翻译选项</p>
      </div>

      <Card title="默认语言" description="设置翻译的默认源语言和目标语言">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">源语言</label>
            <select
              value={config.defaultFrom}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultFrom: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              <option value="auto">自动检测</option>
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">目标语言</label>
            <select
              value={config.defaultTo}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultTo: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </Card>

      <Card title="翻译选项" description="配置翻译行为">
        <div className="space-y-3">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.clipboardMonitor || false}
              onChange={(e) => {
                const enabled = e.target.checked;
                updateConfig((prev) => ({ ...prev, clipboardMonitor: enabled }));
                void saveConfig();
                // Wire preference to real event-driven backend listener
                void syncClipboardMonitorFromConfig(enabled);
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">剪贴板监听</p>
              <p className="text-xs text-text-secondary">
                监听剪贴板变化并自动翻译（Windows 事件驱动；与主界面剪贴板按钮同步）
              </p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.autoCopyResult || false}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, autoCopyResult: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">自动复制结果</p>
              <p className="text-xs text-text-secondary">翻译完成后自动复制到剪贴板</p>
            </div>
          </label>
        </div>
      </Card>
    </div>
  );
}
