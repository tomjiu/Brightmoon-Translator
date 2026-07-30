import { Eye, EyeOff } from 'lucide-react';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';

export default function DeepLXEngineConfig({
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
            type={showSecrets?.deeplx ? 'text' : 'password'}
            value={config.engines.deeplx.apiKey || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  deeplx: {
                    ...prev.engines.deeplx,
                    enabled: prev.engines.deeplx.enabled || false,
                    apiKey: e.target.value,
                    pro: prev.engines.deeplx.pro || false,
                  },
                },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder={t('settings.enginePage.phDeeplx')}
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none pr-10"
          />
          <button
            onClick={() => toggleSecret?.('deeplx')}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
          >
            {showSecrets?.deeplx ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </div>
    </div>
  );
}
