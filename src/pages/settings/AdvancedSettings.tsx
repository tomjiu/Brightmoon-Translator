import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import { isTauriRuntime } from '../../services/tauriRuntime';
import { ExternalLink, CheckCircle, AlertCircle } from 'lucide-react';

export default function AdvancedSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const isTauri = isTauriRuntime();

  return (
    <div className="space-y-5">
      <div>
        <h1 className="text-xl font-semibold text-text-primary">高级设置</h1>
        <p className="text-xs text-text-secondary mt-1">配置高级功能和选项</p>
      </div>

      {isTauri && (
        <Card title="浏览器扩展" description="连接浏览器扩展实现跨平台翻译">
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
                <p className="text-sm font-medium text-text-primary">启用桌面桥接服务</p>
                <p className="text-xs text-text-secondary">
                  允许浏览器扩展连接到桌面应用使用完整功能
                </p>
              </div>
            </label>

            {config.apiServerEnabled && (
              <>
                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    桥接端口
                  </label>
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
                  <p className="text-xs text-text-secondary mt-1">
                    扩展将连接到 http://127.0.0.1:{config.apiServerPort || 60828}
                  </p>
                </div>

                {/* Connection Status */}
                <div className="p-3 bg-bg-secondary rounded-lg border border-border">
                  <div className="flex items-center gap-2 mb-2">
                    <CheckCircle size={16} className="text-success" />
                    <span className="text-sm font-medium text-text-primary">桥接服务运行中</span>
                    <Badge variant="success">在线</Badge>
                  </div>
                  <p className="text-xs text-text-secondary">
                    桌面应用正在监听端口 {config.apiServerPort || 60828}，浏览器扩展可以连接
                  </p>
                </div>
              </>
            )}

            {!config.apiServerEnabled && (
              <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
                <div className="flex items-center gap-2 mb-1">
                  <AlertCircle size={16} className="text-yellow-600 dark:text-yellow-400" />
                  <span className="text-sm font-medium text-yellow-600 dark:text-yellow-400">
                    桥接服务未启用
                  </span>
                </div>
                <p className="text-xs text-text-secondary">
                  扩展将使用本地模式，功能受限（无桌面引擎、无本地OCR、无术语表同步）
                </p>
              </div>
            )}

            {/* Download Links */}
            <div className="border-t border-border pt-4">
              <p className="text-sm font-medium text-text-primary mb-3">下载浏览器扩展</p>
              <div className="space-y-2">
                <a
                  href="https://chrome.google.com/webstore"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex items-center justify-between p-3 bg-bg-secondary hover:bg-bg-tertiary rounded-lg border border-border transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded bg-gradient-to-br from-red-500 to-yellow-500 flex items-center justify-center text-white font-bold text-xs">
                      Cr
                    </div>
                    <div>
                      <p className="text-sm font-medium text-text-primary">Chrome / Edge 扩展</p>
                      <p className="text-xs text-text-secondary">适用于 Chromium 内核浏览器</p>
                    </div>
                  </div>
                  <ExternalLink size={16} className="text-text-secondary" />
                </a>

                <a
                  href="https://addons.mozilla.org"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="flex items-center justify-between p-3 bg-bg-secondary hover:bg-bg-tertiary rounded-lg border border-border transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded bg-gradient-to-br from-orange-500 to-purple-600 flex items-center justify-center text-white font-bold text-xs">
                      Fx
                    </div>
                    <div>
                      <p className="text-sm font-medium text-text-primary">Firefox 扩展</p>
                      <p className="text-xs text-text-secondary">适用于 Firefox 浏览器</p>
                    </div>
                  </div>
                  <ExternalLink size={16} className="text-text-secondary" />
                </a>
              </div>

              <p className="text-xs text-text-secondary mt-3">
                安装扩展后，它会自动尝试连接到桌面应用。确保桌面应用正在运行且桥接服务已启用。
              </p>
            </div>
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
