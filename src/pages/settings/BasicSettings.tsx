import { useConfigStore } from '../../stores/configStore';
import { useTranslateStore } from '../../stores/translateStore';
import Card from '../../components/Card';
import { LANGUAGES } from '../../types';
import { useI18n } from '../../i18n';

export default function BasicSettings() {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const syncClipboardMonitorFromConfig = useTranslateStore((s) => s.syncClipboardMonitorFromConfig);

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.basic.title')}</h1>
        <p className="ui-page-desc">{t('settings.basic.desc')}</p>
      </div>

      <Card title={t('settings.basic.batchTitle')} description={t('settings.basic.batchDesc')}>
        <div>
          <label className="block text-sm font-medium text-text-primary mb-2">
            {t('settings.basic.batchEngine')}
          </label>
          <input
            value={config.batchPreferredEngine || ''}
            onChange={(e) => {
              updateConfig((prev) => ({ ...prev, batchPreferredEngine: e.target.value.trim() }));
              void saveConfig();
            }}
            placeholder={t('settings.basic.batchEnginePh')}
            list="batch-engines"
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm font-mono"
          />
          <datalist id="batch-engines">
            {['offline', 'Google', 'Youdao', 'DeepL', 'DeepLX', 'Baidu', 'Microsoft', 'Caiyun', 'Yandex', 'LLM'].map(
              (name) => (
                <option key={name} value={name} />
              ),
            )}
          </datalist>
          <p className="text-xs text-text-secondary mt-1">{t('settings.basic.batchEngineHint')}</p>
        </div>
      </Card>

      <Card
        title={t('settings.basic.defaultLangTitle')}
        description={t('settings.basic.defaultLangDesc')}
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.basic.sourceLang')}
            </label>
            <select
              value={config.defaultFrom}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultFrom: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              <option value="auto">{t('settings.basic.autoDetect')}</option>
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.basic.targetLang')}
            </label>
            <select
              value={config.defaultTo}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, defaultTo: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            >
              {LANGUAGES.map((lang) => (
                <option key={lang.code} value={lang.code}>
                  {lang.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </Card>

      <Card title={t('settings.basic.optionsTitle')} description={t('settings.basic.optionsDesc')}>
        <div className="space-y-3">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.clipboardMonitor || false}
              onChange={(e) => {
                const enabled = e.target.checked;
                updateConfig((prev) => ({ ...prev, clipboardMonitor: enabled }));
                void saveConfig();
                // Wire preference to real event-driven backend listener
                void syncClipboardMonitorFromConfig(enabled);
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">
                {t('settings.basic.clipboardMonitor')}
              </p>
              <p className="text-xs text-text-secondary">
                {t('settings.basic.clipboardMonitorHint')}
              </p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.autoCopyResult || false}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, autoCopyResult: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">{t('settings.basic.autoCopy')}</p>
              <p className="text-xs text-text-secondary">{t('settings.basic.autoCopyHint')}</p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.useClipboardOutput}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, useClipboardOutput: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">
                {t('settings.basic.clipboardPaste')}
              </p>
              <p className="text-xs text-text-secondary">{t('settings.basic.clipboardPasteHint')}</p>
            </div>
          </label>

          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.ttsAutoPlay || false}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, ttsAutoPlay: e.target.checked }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">{t('settings.basic.ttsAuto')}</p>
              <p className="text-xs text-text-secondary">{t('settings.basic.ttsAutoHint')}</p>
            </div>
          </label>
        </div>
      </Card>

      <Card title={t('settings.basic.ttsTitle')} description={t('settings.basic.ttsDesc')}>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.basic.ttsBackend')}
            </label>
            <select
              value={config.ttsProvider || 'edge'}
              onChange={(e) => {
                updateConfig((prev) => ({ ...prev, ttsProvider: e.target.value }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg outline-none"
            >
              <option value="edge">{t('settings.basic.ttsEdge')}</option>
              <option value="fish">{t('settings.basic.ttsFish')}</option>
              <option value="openai">{t('settings.basic.ttsOpenai')}</option>
              <option value="youdao">{t('settings.basic.ttsYoudao')}</option>
            </select>
          </div>

          {(config.ttsProvider || 'edge') === 'edge' && (
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.basic.ttsVoice')}
              </label>
              <input
                value={config.ttsVoice || ''}
                onChange={(e) => {
                  updateConfig((prev) => ({ ...prev, ttsVoice: e.target.value }));
                  void saveConfig();
                }}
                placeholder="zh-CN-XiaoxiaoNeural"
                className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
              />
            </div>
          )}

          {(config.ttsProvider || 'edge') === 'fish' && (
            <div className="space-y-3">
              <p className="text-xs text-text-secondary">{t('settings.basic.fishHint')}</p>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
                <input
                  type="password"
                  value={config.fishTts?.apiKey || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      fishTts: {
                        apiKey: e.target.value,
                        model: prev.fishTts?.model || 's2.1-pro-free',
                        referenceId:
                          prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                        format: prev.fishTts?.format || 'mp3',
                        speed: prev.fishTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                  placeholder="Fish API Key"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Model</label>
                  <select
                    value={config.fishTts?.model || 's2.1-pro-free'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        fishTts: {
                          apiKey: prev.fishTts?.apiKey || '',
                          model: e.target.value,
                          referenceId:
                            prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                          format: prev.fishTts?.format || 'mp3',
                          speed: prev.fishTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  >
                    <option value="s2.1-pro-free">{t('settings.basic.fishFree')}</option>
                    <option value="s2.1-pro">s2.1-pro</option>
                    <option value="s2-pro">s2-pro</option>
                    <option value="s1">s1</option>
                  </select>
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">
                    {t('settings.basic.speed')}
                  </label>
                  <input
                    type="number"
                    min={0.5}
                    max={2}
                    step={0.1}
                    value={config.fishTts?.speed ?? 1}
                    onChange={(e) => {
                      const speed = Number(e.target.value) || 1;
                      updateConfig((prev) => ({
                        ...prev,
                        fishTts: {
                          apiKey: prev.fishTts?.apiKey || '',
                          model: prev.fishTts?.model || 's2.1-pro-free',
                          referenceId:
                            prev.fishTts?.referenceId || '12b8a0bf8e0042c3b11e519d11db8b68',
                          format: prev.fishTts?.format || 'mp3',
                          speed,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  />
                </div>
              </div>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">
                  {t('settings.basic.fishRef')}
                </label>
                <input
                  value={config.fishTts?.referenceId || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      fishTts: {
                        apiKey: prev.fishTts?.apiKey || '',
                        model: prev.fishTts?.model || 's2.1-pro-free',
                        referenceId: e.target.value,
                        format: prev.fishTts?.format || 'mp3',
                        speed: prev.fishTts?.speed ?? 1,
                      },
                      ttsVoice: e.target.value,
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm font-mono"
                  placeholder="12b8a0bf8e0042c3b11e519d11db8b68"
                />
              </div>
            </div>
          )}

          {(config.ttsProvider || 'edge') === 'openai' && (
            <div className="space-y-3">
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
                <input
                  type="password"
                  value={config.openaiTts?.apiKey || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      openaiTts: {
                        apiKey: e.target.value,
                        baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                        model: prev.openaiTts?.model || 'tts-1',
                        voice: prev.openaiTts?.voice || 'alloy',
                        speed: prev.openaiTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                  placeholder="sk-..."
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">Base URL</label>
                <input
                  value={config.openaiTts?.baseUrl || 'https://api.openai.com/v1'}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      openaiTts: {
                        apiKey: prev.openaiTts?.apiKey || '',
                        baseUrl: e.target.value,
                        model: prev.openaiTts?.model || 'tts-1',
                        voice: prev.openaiTts?.voice || 'alloy',
                        speed: prev.openaiTts?.speed ?? 1,
                      },
                    }));
                    void saveConfig();
                  }}
                  className="w-full px-3 py-2 bg-bg-tertiary border border-border rounded-lg text-sm"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Model</label>
                  <input
                    value={config.openaiTts?.model || 'tts-1'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        openaiTts: {
                          apiKey: prev.openaiTts?.apiKey || '',
                          baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                          model: e.target.value,
                          voice: prev.openaiTts?.voice || 'alloy',
                          speed: prev.openaiTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs text-text-secondary mb-1">Voice</label>
                  <input
                    value={config.openaiTts?.voice || 'alloy'}
                    onChange={(e) => {
                      updateConfig((prev) => ({
                        ...prev,
                        openaiTts: {
                          apiKey: prev.openaiTts?.apiKey || '',
                          baseUrl: prev.openaiTts?.baseUrl || 'https://api.openai.com/v1',
                          model: prev.openaiTts?.model || 'tts-1',
                          voice: e.target.value,
                          speed: prev.openaiTts?.speed ?? 1,
                        },
                      }));
                      void saveConfig();
                    }}
                    className="w-full px-2 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
                    placeholder="alloy / echo / ..."
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
