import { ExternalLink, Eye, EyeOff } from 'lucide-react';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';

export default function CaiyunEngineConfig({
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
        <label className="block text-sm font-medium text-text-primary mb-2">API Token</label>
        <div className="relative">
          <input
            type={showSecrets?.caiyun ? 'text' : 'password'}
            value={config.engines.caiyun?.apiToken || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  caiyun: {
                    ...prev.engines.caiyun,
                    enabled: prev.engines.caiyun?.enabled || false,
                    apiToken: e.target.value,
                  },
                },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder={t('settings.enginePage.phCaiyun')}
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none pr-10"
          />
          <button
            onClick={() => toggleSecret?.('caiyun')}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
          >
            {showSecrets?.caiyun ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </div>
      <a
        href="https://dashboard.caiyunapp.com/user/sign_in/"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
      >
        {t('settings.enginePage.getToken')} <ExternalLink size={12} />
      </a>
    </div>
  );
}
