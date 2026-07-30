import { ExternalLink, Eye, EyeOff } from 'lucide-react';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';

export default function BaiduEngineConfig({
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
        <label className="block text-sm font-medium text-text-primary mb-2">APP ID</label>
        <input
          type="text"
          value={config.engines.baidu.appId || ''}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: { ...prev.engines, baidu: { ...prev.engines.baidu, appId: e.target.value } },
            }));
          }}
          onBlur={() => void saveConfig()}
          placeholder={t('settings.enginePage.phBaiduId')}
          className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
        />
      </div>
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">
          {t('settings.enginePage.secret')}
        </label>
        <div className="relative">
          <input
            type={showSecrets?.baidu ? 'text' : 'password'}
            value={config.engines.baidu.secret || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  baidu: { ...prev.engines.baidu, secret: e.target.value },
                },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder={t('settings.enginePage.phBaiduSecret')}
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none pr-10"
          />
          <button
            onClick={() => toggleSecret?.('baidu')}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
          >
            {showSecrets?.baidu ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </div>
      <a
        href="https://fanyi-api.baidu.com/product/11"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
      >
        {t('settings.enginePage.getCreds')} <ExternalLink size={12} />
      </a>
    </div>
  );
}
