import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import { isTauriRuntime } from '../../services/tauriRuntime';
import { ExternalLink, CheckCircle, AlertCircle } from 'lucide-react';
import { useI18n } from '../../i18n';

export default function AdvancedSettings() {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const isTauri = isTauriRuntime();

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.advanced.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.advanced.pageDesc')}</p>
      </div>

      {isTauri && (
        <Card title={t('settings.advanced.extTitle')} description={t('settings.advanced.extDesc')}>
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
                <p className="text-sm font-medium text-text-primary">
                  {t('settings.advanced.apiEnable')}
                </p>
                <p className="ui-caption">{t('settings.advanced.apiEnableHint')}</p>
              </div>
            </label>

            {config.apiServerEnabled && (
              <>
                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    {t('settings.advanced.apiPort')}
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
                    {t('settings.advanced.apiPortHint', {
                      port: config.apiServerPort || 60828,
                    })}
                  </p>
                </div>

                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    {t('settings.advanced.apiToken')}
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
                      placeholder={t('settings.advanced.apiTokenPh')}
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
                      {t('settings.advanced.regen')}
                    </button>
                    <button
                      type="button"
                      className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary"
                      onClick={() => {
                        const t = config.apiServerToken || '';
                        if (t) void navigator.clipboard.writeText(t);
                      }}
                    >
                      {t('settings.advanced.copy')}
                    </button>
                  </div>
                  <p className="ui-caption mt-1">{t('settings.advanced.tokenHint2')}</p>
                </div>

                <div className="p-3 bg-bg-secondary rounded-lg border border-border">
                  <div className="flex items-center gap-2 mb-2">
                    <CheckCircle size={16} className="text-success" />
                    <span className="text-sm font-medium text-text-primary">
                      {t('settings.advanced.bridgeOn')}
                    </span>
                    <Badge variant="success">{t('settings.advanced.auth')}</Badge>
                  </div>
                  <p className="ui-caption">
                    {t('settings.advanced.bridgePortNote', {
                      port: config.apiServerPort || 60828,
                    })}
                  </p>
                </div>
              </>
            )}

            {!config.apiServerEnabled && (
              <div className="p-3 bg-yellow-500/10 border border-yellow-500/30 rounded-lg">
                <div className="flex items-center gap-2 mb-1">
                  <AlertCircle size={16} className="text-yellow-600 dark:text-yellow-400" />
                  <span className="text-sm font-medium text-yellow-600 dark:text-yellow-400">
                    {t('settings.advanced.bridgeOff')}
                  </span>
                </div>
                <p className="text-xs text-text-secondary">{t('settings.advanced.bridgeOffHint')}</p>
              </div>
            )}

            {/* Download Links */}
            <div className="border-t border-border pt-4">
              <p className="text-sm font-medium text-text-primary mb-3">
                {t('settings.advanced.downloadExt')}
              </p>
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
                      <p className="text-sm font-medium text-text-primary">
                        {t('settings.advanced.chrome')}
                      </p>
                      <p className="text-xs text-text-secondary">
                        {t('settings.advanced.chromeHint')}
                      </p>
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
                      <p className="text-sm font-medium text-text-primary">
                        {t('settings.advanced.firefox')}
                      </p>
                      <p className="text-xs text-text-secondary">
                        {t('settings.advanced.firefoxHint')}
                      </p>
                    </div>
                  </div>
                  <ExternalLink size={16} className="text-text-secondary" />
                </a>
              </div>

              <p className="text-xs text-text-secondary mt-3">
                {t('settings.advanced.extInstallHint')}
              </p>
            </div>
          </div>
        </Card>
      )}

      <Card
        title={t('settings.advanced.proxyTitle')}
        description={t('settings.advanced.proxyDesc')}
      >
        <div className="space-y-4">
          <p className="ui-caption">{t('settings.advanced.proxyNote')}</p>
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
              <p className="text-sm font-medium text-text-primary">
                {t('settings.advanced.proxyEnable')}
              </p>
              <p className="ui-caption">{t('settings.advanced.proxyEnableHint')}</p>
            </div>
          </label>

          {config.proxy.enabled && (
            <div className="space-y-3 pl-8">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    {t('settings.advanced.proxyHost')}
                  </label>
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
                  <label className="block text-sm font-medium text-text-primary mb-2">
                    {t('settings.advanced.proxyPort')}
                  </label>
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
