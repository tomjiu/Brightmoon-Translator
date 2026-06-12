import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import { isTauriRuntime } from '../../services/tauriRuntime';

export default function AdvancedSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const isTauri = isTauriRuntime();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-text-primary">高级设置</h1>
        <p className="text-sm text-text-secondary mt-1">配置高级功能和选项</p>
      </div>

      {isTauri && (
        <Card title="API 服务器" description="启用本地 API 服务器供浏览器扩展连接">
          <div className="space-y-4">
            <label className="flex items-center gap-3">
              <input
                type="checkbox"
                checked={config.apiServerEnabled || false}
                onChange={(e) => {
                  updateConfig((prev) => ({ ...prev, apiServerEnabled: e.target.checked }));
                  void saveConfig();
                }}
                className="rounded"
              />
              <div>
                <p className="text-sm font-medium text-text-primary">启用 API 服务器</p>
                <p className="text-xs text-text-secondary">允许浏览器扩展连接到桌面应用</p>
              </div>
            </label>

            {config.apiServerEnabled && (
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">端口号</label>
                <input
                  type="number"
                  min="1024"
                  max="65535"
                  value={config.apiServerPort || 60828}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      apiServerPort: parseInt(e.target.value, 10),
                    }));
                  }}
                  onBlur={() => void saveConfig()}
                  className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
                />
              </div>
            )}
          </div>
        </Card>
      )}

      <Card title="代理设置" description="配置网络代理">
        <div className="space-y-4">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.proxy.enabled || false}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  proxy: { ...prev.proxy, enabled: e.target.checked },
                }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">启用代理</p>
              <p className="text-xs text-text-secondary">通过代理服务器访问翻译 API</p>
            </div>
          </label>

          {config.proxy.enabled && (
            <div className="space-y-3 pl-8">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">主机</label>
                  <input
                    type="text"
                    value={config.proxy.host || ''}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        proxy: { ...prev.proxy, host: e.target.value },
                      }));
                    }}
                    onBlur={() => void saveConfig()}
                    placeholder="127.0.0.1"
                    className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
                  />
                </div>

                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">端口</label>
                  <input
                    type="number"
                    min="1"
                    max="65535"
                    value={config.proxy.port || 7890}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        proxy: { ...prev.proxy, port: parseInt(e.target.value, 10) },
                      }));
                    }}
                    onBlur={() => void saveConfig()}
                    className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
