import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';

export default function OfflineEngineConfig({
  config,
  updateConfig,
  saveConfig,
}: EngineConfigProps) {
  const { t } = useI18n();
  const offline = config.engines.offline ?? {
    enabled: false,
    autoSwitch: true,
    downloadedModels: [],
    modelDir: '',
  };
  const modelCount = (offline.downloadedModels ?? []).length;
  return (
    <div className="mt-3 space-y-2">
      <p className="ui-caption">
        {t('settings.enginePage.downloadedModels', { count: modelCount })}
      </p>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={offline.autoSwitch || false}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                offline: {
                  enabled: prev.engines.offline.enabled ?? false,
                  autoSwitch: e.target.checked,
                  downloadedModels: prev.engines.offline.downloadedModels ?? [],
                  modelDir: prev.engines.offline.modelDir ?? '',
                },
              },
            }));
            void saveConfig();
          }}
          className="rounded"
        />
        <span className="ui-body text-text-secondary">{t('settings.enginePage.autoOffline')}</span>
      </label>
    </div>
  );
}
