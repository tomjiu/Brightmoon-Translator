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
        <h1 className="ui-page-title">高级设置</h1>
        <p className="ui-page-desc">配置高级功能和选项</p>
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

                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    API 令牌（Bearer）
                  </label>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={config.apiServerToken || ''}
                      onChange={(e) => {
                        updateConfig((prev) => ({
                          ...prev,
                          apiServerToken: e.target.value.trim(),
                        }));
                      }}
                      onBlur={() => void saveConfig()}
                      placeholder="启用后若为空会自动生成"
                      className="flex-1 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none font-mono text-xs"
                    />
                    <button
                      type="button"
                      className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary"
                      onClick={() => {
                        const token =
                          typeof crypto !== 'undefined' && crypto.randomUUID
                            ? crypto.randomUUID()
                            : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
                        updateConfig((prev) => ({ ...prev, apiServerToken: token }));
                        void saveConfig();
                      }}
                    >
                      重新生成
                    </button>
                    <button
                      type="button"
                      className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary"
                      onClick={() => {
                        const t = config.apiServerToken || '';
                        if (t) void navigator.clipboard.writeText(t);
                      }}
                    >
                      复制
                    </button>
                  </div>
                  <p className="text-xs text-text-secondary mt-1">
                    请求头：Authorization: Bearer &lt;令牌&gt; 或 X-Api-Token。扩展需在存储中配置
                    desktopApiToken。
                  </p>
                </div>

                <div className="p-3 bg-bg-secondary rounded-lg border border-border">
                  <div className="flex items-center gap-2 mb-2">
                    <CheckCircle size={16} className="text-success" />
                    <span className="text-sm font-medium text-text-primary">
                      桥接已启用（需重启应用后监听生效）
                    </span>
                    <Badge variant="success">鉴权</Badge>
                  </div>
                  <p className="text-xs text-text-secondary">
                    除 GET /health 外均需令牌。端口 {config.apiServerPort || 60828}
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
                    <div className="w-8 h-8 rounded bg-gradient-to-br from-neutral-500 to-neutral-600 flex items-center justify-center text-white font-bold text-xs">
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
