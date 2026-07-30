import { ExternalLink, Eye, EyeOff } from 'lucide-react';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';

export default function DeepLEngineConfig({
  config,
  updateConfig,
  saveConfig,
  showSecrets,
  toggleSecret,
}: EngineConfigProps) {
  const { t } = useI18n();
  return (
    <div className="mt-3 space-y-3">
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
        <div className="relative">
          <input
            type={showSecrets?.deepl ? 'text' : 'password'}
            value={config.engines.deepl.apiKey || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  deepl: {
                    ...prev.engines.deepl,
                    enabled: prev.engines.deepl.enabled || false,
                    apiKey: e.target.value,
                    pro: prev.engines.deepl.pro || false,
                  },
                },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder={t('settings.enginePage.phDeepl')}
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none pr-10"
          />
          <button
            onClick={() => toggleSecret?.('deepl')}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
          >
            {showSecrets?.deepl ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </div>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={config.engines.deepl.pro || false}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                deepl: {
                  ...prev.engines.deepl,
                  enabled: prev.engines.deepl.enabled || false,
                  apiKey: prev.engines.deepl.apiKey || '',
                  pro: e.target.checked,
                },
              },
            }));
            void saveConfig();
          }}
          className="rounded"
        />
        <span className="text-sm text-text-secondary">{t('settings.enginePage.proAccount')}</span>
      </label>
      <a
        href="https://www.deepl.com/pro-api"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
      >
        {t('settings.enginePage.getApiKey')} <ExternalLink size={12} />
      </a>
    </div>
  );
}
