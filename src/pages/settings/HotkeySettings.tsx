import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import { isTauriRuntime } from '../../services/tauriRuntime';

export default function HotkeySettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const isTauri = isTauriRuntime();

  if (!isTauri) {
    return (
      <div className="space-y-6">
        <div>
          <h1 className="text-2xl font-bold text-text-primary">快捷键设置</h1>
          <p className="text-sm text-text-secondary mt-1">快捷键仅在桌面版可用</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-xl font-semibold text-text-primary">快捷键设置</h1>
        <p className="text-xs text-text-secondary mt-1">配置全局快捷键</p>
      </div>

      <Card title="全局快捷键" description="设置系统级快捷键">
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">OCR 截图翻译</label>
            <input
              type="text"
              value={config.hotkeys.ocrTranslate || 'Ctrl+Shift+T'}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  hotkeys: { ...prev.hotkeys, ocrTranslate: e.target.value },
                }));
              }}
              onBlur={() => void saveConfig()}
              placeholder="Ctrl+Shift+T"
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">显示主窗口</label>
            <input
              type="text"
              value={config.hotkeys.showWindow || 'Ctrl+T'}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  hotkeys: { ...prev.hotkeys, showWindow: e.target.value },
                }));
              }}
              onBlur={() => void saveConfig()}
              placeholder="Ctrl+T"
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">选中文本翻译</label>
            <input
              type="text"
              value={config.hotkeys.translateSelection || 'Ctrl+Shift+Y'}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  hotkeys: { ...prev.hotkeys, translateSelection: e.target.value },
                }));
              }}
              onBlur={() => void saveConfig()}
              placeholder="Ctrl+Shift+Y"
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
            />
          </div>
        </div>
      </Card>
    </div>
  );
}
