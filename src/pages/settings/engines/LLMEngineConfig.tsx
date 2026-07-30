import { ExternalLink } from 'lucide-react';
import { useI18n } from '../../../i18n';
import { isLlmConfigured } from './enginesMeta';
import type { EngineConfigProps } from './types';

export default function LLMEngineConfig({ config, onNavigate }: EngineConfigProps) {
  const llm = config.llm ?? {
    provider: '',
    apiKey: '',
    apiKeys: [],
    baseUrl: '',
    model: '',
    providers: [],
  };
  const configured = isLlmConfigured(llm);
  const { t } = useI18n();
  const model = (llm.model ?? '').trim() || t('settings.enginePage.noModel');
  const provider = llm.provider || 'custom';
  return (
    <div className="mt-3 space-y-2">
      <p className="text-sm text-text-secondary">
        {configured
          ? t('settings.enginePage.statusConfigured', { provider, model })
          : t('settings.enginePage.statusNeedKey')}
      </p>
      {onNavigate ? (
        <button
          type="button"
          onClick={() => onNavigate('ai')}
          className="inline-flex items-center gap-1 text-sm font-medium text-primary hover:underline"
        >
          {t('settings.enginePage.goAiConfig')}
          <ExternalLink size={14} />
        </button>
      ) : (
        <p className="text-sm text-primary">{t('settings.enginePage.goAiConfig')}</p>
      )}
    </div>
  );
}
