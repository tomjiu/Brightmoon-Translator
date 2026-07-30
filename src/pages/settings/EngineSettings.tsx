import { useState, useCallback, useMemo } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { RoutingStrategy, AppConfig } from '../../types';
import { ROUTING_STRATEGIES } from './engines/routingStrategies';
import {
  DEFAULT_ENGINE_ORDER,
  ENGINE_SECTIONS,
  enginesInSection,
  isLlmConfigured,
} from './engines/enginesMeta';
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';
import {
  AlertCircle,
  CheckCircle,
  ExternalLink,
  Eye,
  EyeOff,
  ChevronUp,
  ChevronDown,
  Bot,
  Languages,
} from 'lucide-react';
import { useI18n } from '../../i18n';

type ConfigUpdater = (updater: (prev: AppConfig) => AppConfig) => void;

interface EngineBadge {
  label: string;
  variant: 'success' | 'warning' | 'error' | 'info';
}

interface EngineDisplayConfig {
  id: string;
  name: string;
  enabled: boolean;
  status: 'connected' | 'warning' | 'error';
  badges: EngineBadge[];
  description: string;
}

interface EngineSettingsProps {
  /** Jump to another settings section (e.g. ai / ocr) */
  onNavigate?: (sectionId: string) => void;
}

export default function EngineSettings({ onNavigate }: EngineSettingsProps) {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const [showSecrets, setShowSecrets] = useState<Record<string, boolean>>({});

  const toggleSecret = (key: string) => {
    setShowSecrets((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  // Merge saved order with any new engine ids
  const engineOrder = useMemo(() => {
    const saved = config.engineOrder?.filter(Boolean) ?? [];
    const base = saved.length > 0 ? saved : DEFAULT_ENGINE_ORDER;
    const missing = DEFAULT_ENGINE_ORDER.filter((id) => !base.includes(id));
    return [...base, ...missing];
  }, [config.engineOrder]);

  const persistEngineOrder = useCallback(
    (newOrder: string[]) => {
      updateConfig((prev) => ({ ...prev, engineOrder: newOrder }));
      void saveConfig();
    },
    [updateConfig, saveConfig],
  );

  const moveEngine = useCallback(
    (idx: number, direction: 'up' | 'down') => {
      const next = direction === 'up' ? idx - 1 : idx + 1;
      if (next < 0 || next >= engineOrder.length) return;
      const order = [...engineOrder];
      [order[idx], order[next]] = [order[next], order[idx]];
      persistEngineOrder(order);
    },
    [engineOrder, persistEngineOrder],
  );

  const rawStrategy = config.routingStrategy || 'fallback_on_error';
  const currentStrategy = ROUTING_STRATEGIES.some((s) => s.id === rawStrategy)
    ? rawStrategy
    : 'fallback_on_error';

  // 引擎配置映射 - 根据ID返回引擎配置
  const getEngineConfig = (engineId: string): EngineDisplayConfig | null => {
    switch (engineId) {
      case 'llm': {
        const llmOk = isLlmConfigured(config.llm);
        return {
          id: 'llm',
          name: t('settings.enginePage.llmName'),
          enabled: llmOk,
          status: (llmOk ? 'connected' : 'warning') as 'connected' | 'warning' | 'error',
          badges: [
            { label: t('settings.enginePage.badgePrimary'), variant: 'info' as const },
            llmOk
              ? { label: t('settings.enginePage.badgeConfigured'), variant: 'success' as const }
              : { label: t('settings.enginePage.badgeNeedKey'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.llmDesc'),
        };
      }
      case 'google':
        return {
          id: 'google',
          name: 'Google Translation',
          enabled: config.engines.google.enabled,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeNoConfig'), variant: 'info' as const },
          ],
          description: t('settings.enginePage.googleDesc'),
        };
      case 'youdao':
        return {
          id: 'youdao',
          name: t('settings.enginePage.youdaoName'),
          enabled: config.engines.youdao.enabled,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeWebFree'), variant: 'info' as const },
          ],
          description: t('settings.enginePage.youdaoDesc'),
        };
      case 'caiyun': {
        const hasToken = !!config.engines.caiyun?.apiToken.trim();
        return {
          id: 'caiyun',
          name: t('settings.enginePage.caiyunName'),
          enabled: config.engines.caiyun?.enabled || false,
          status: hasToken ? 'connected' : 'warning',
          badges: [
            { label: t('settings.enginePage.badgeFreeQuota'), variant: 'success' as const },
            hasToken
              ? { label: t('settings.enginePage.badgeConfigured'), variant: 'success' as const }
              : { label: t('settings.enginePage.badgeNeedKeyRoute'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.caiyunDesc'),
        };
      }
      case 'deepl': {
        const hasKey = !!config.engines.deepl.apiKey.trim();
        return {
          id: 'deepl',
          name: 'DeepL',
          enabled: config.engines.deepl.enabled || false,
          status: hasKey ? 'connected' : 'warning',
          badges: [
            { label: t('settings.enginePage.badgePaid'), variant: 'warning' as const },
            hasKey
              ? { label: t('settings.enginePage.badgeConfigured'), variant: 'success' as const }
              : { label: t('settings.enginePage.badgeNeedKeyRoute'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.deeplDesc'),
        };
      }
      case 'deeplx':
        return {
          id: 'deeplx',
          name: 'DeepLX',
          enabled: config.engines.deeplx.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeOptionalKey'), variant: 'info' as const },
          ],
          description: t('settings.enginePage.deeplxDesc'),
        };
      case 'baidu': {
        const hasAppId = !!config.engines.baidu.appId.trim();
        return {
          id: 'baidu',
          name: t('settings.enginePage.baiduName'),
          enabled: config.engines.baidu.enabled,
          status: hasAppId ? 'connected' : 'warning',
          badges: [
            { label: t('settings.enginePage.badgeFreeQuota'), variant: 'success' as const },
            hasAppId
              ? { label: t('settings.enginePage.badgeConfigured'), variant: 'success' as const }
              : { label: t('settings.enginePage.badgeNeedKeyRoute'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.baiduDesc'),
        };
      }
      case 'microsoft':
        return {
          id: 'microsoft',
          name: t('settings.enginePage.msName'),
          enabled: config.engines.microsoft.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeNoConfig'), variant: 'info' as const },
          ],
          description: t('settings.enginePage.msDesc'),
        };
      case 'yandex':
        return {
          id: 'yandex',
          name: t('settings.enginePage.yandexName'),
          enabled: config.engines.yandex.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeNoConfig'), variant: 'info' as const },
          ],
          description: t('settings.enginePage.yandexDesc'),
        };
      case 'offline':
        return {
          id: 'offline',
          name: t('settings.enginePage.offlineName'),
          enabled: config.engines.offline.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeLocal'), variant: 'info' as const },
            { label: t('settings.enginePage.badgeNeedModel'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.offlineDesc'),
        };
      case 'tatoeba':
        return {
          id: 'tatoeba',
          name: t('settings.enginePage.tatoebaName'),
          enabled: config.engines.tatoeba?.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: t('settings.enginePage.badgeExample'), variant: 'info' as const },
            { label: t('settings.enginePage.badgeNotMt'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.tatoebaDesc'),
        };
      case 'baidu_web':
        return {
          id: 'baidu_web',
          name: t('settings.enginePage.baiduWebName'),
          enabled: config.engines.baiduWeb?.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeUnofficial'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.baiduWebDesc'),
        };
      case 'caiyun_web':
        return {
          id: 'caiyun_web',
          name: t('settings.enginePage.caiyunWebName'),
          enabled: config.engines.caiyunWeb?.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeUnofficial'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.caiyunWebDesc'),
        };
      case 'volcengine_web':
        return {
          id: 'volcengine_web',
          name: t('settings.enginePage.volcName'),
          enabled: config.engines.volcengineWeb?.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeUnofficial'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.volcDesc'),
        };
      case 'transmart':
        return {
          id: 'transmart',
          name: t('settings.enginePage.transmartName'),
          enabled: config.engines.transmart?.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeUnofficial'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.transmartDesc'),
        };
      case 'papago':
        return {
          id: 'papago',
          name: 'Papago',
          enabled: config.engines.papago?.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: t('settings.enginePage.badgeFree'), variant: 'success' as const },
            { label: t('settings.enginePage.badgeUnofficial'), variant: 'warning' as const },
          ],
          description: t('settings.enginePage.papagoDesc'),
        };
      default:
        return null;
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.enginePage.title')}</h1>
        <p className="ui-page-desc">{t('settings.enginePage.desc')}</p>
      </div>

      <Card
        title={t('settings.enginePage.usageTitle')}
        description={t('settings.enginePage.usageDesc')}
      >
        <ul className="text-xs text-text-secondary space-y-1.5 leading-relaxed list-disc pl-4">
          <li>
            <span className="text-text-primary font-medium">
              {t('settings.enginePage.usageMain')}
            </span>
            {t('settings.enginePage.usageMainDesc')}
          </li>
          <li>
            <span className="text-text-primary font-medium">
              {t('settings.enginePage.usageOcr')}
            </span>
            {t('settings.enginePage.usageOcrDesc')}
          </li>
          <li>
            <span className="text-text-primary font-medium">
              {t('settings.enginePage.usageHook')}
            </span>
            {t('settings.enginePage.usageHookDesc')}
          </li>
        </ul>
        <div className="flex flex-wrap gap-2 mt-3">
          {onNavigate && (
            <>
              <button
                type="button"
                onClick={() => onNavigate('ai')}
                className="inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline"
              >
                {t('settings.enginePage.goAi')}
                <ExternalLink size={12} />
              </button>
              <button
                type="button"
                onClick={() => onNavigate('ocr')}
                className="inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline"
              >
                {t('settings.enginePage.goOcr')}
                <ExternalLink size={12} />
              </button>
            </>
          )}
        </div>
      </Card>

      <Card
        title={t('settings.enginePage.routingTitle')}
        description={t('settings.enginePage.routingDesc')}
      >
        <div className="grid gap-2">
          {ROUTING_STRATEGIES.map((strategy) => (
            <label
              key={strategy.id}
              className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors ${
                currentStrategy === strategy.id
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-border-strong'
              }`}
            >
              <input
                type="radio"
                name="routingStrategy"
                value={strategy.id}
                checked={currentStrategy === strategy.id}
                onChange={(e) => {
                  updateConfig((prev) => ({
                    ...prev,
                    routingStrategy: e.target.value as RoutingStrategy,
                  }));
                  void saveConfig();
                }}
                className="mt-1"
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-text-primary">
                    {t(strategy.labelKey)}
                  </span>
                  {strategy.recommended && (
                    <Badge variant="info">{t('settings.enginePage.defaultBadge')}</Badge>
                  )}
                </div>
                <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
                  {t(strategy.descriptionKey)}
                </p>
              </div>
            </label>
          ))}
        </div>
      </Card>

      <p className="text-xs text-text-secondary leading-relaxed">
        {t('settings.enginePage.listHint')}
      </p>

      {ENGINE_SECTIONS.map((section) => {
        const sectionIds = enginesInSection(engineOrder, section.id);
        if (sectionIds.length === 0) return null;

        return (
          <Card key={section.id} title={t(section.title)} description={t(section.description)}>
            <div className="space-y-2">
              {sectionIds.map((engineId) => {
                const idx = engineOrder.indexOf(engineId);
                const engineConfig = getEngineConfig(engineId);
                if (!engineConfig || idx < 0) return null;

                return (
                  <SortableEngineCard
                    key={engineId}
                    engineId={engineId}
                    engineConfig={engineConfig}
                    config={config}
                    updateConfig={updateConfig}
                    saveConfig={saveConfig}
                    showSecrets={showSecrets}
                    toggleSecret={toggleSecret}
                    index={idx}
                    total={engineOrder.length}
                    onMoveUp={() => moveEngine(idx, 'up')}
                    onMoveDown={() => moveEngine(idx, 'down')}
                    onNavigate={onNavigate}
                  />
                );
              })}
            </div>
            {section.id === 'offline' && (
              <p className="mt-3 text-xs text-text-secondary leading-relaxed">
                {t('settings.enginePage.offlineOcrNote')}
              </p>
            )}
          </Card>
        );
      })}
    </div>
  );
}

// 各引擎子配置组件
interface EngineConfigProps {
  config: AppConfig;
  updateConfig: ConfigUpdater;
  saveConfig: () => Promise<void>;
  showSecrets?: Record<string, boolean>;
  toggleSecret?: (key: string) => void;
  onNavigate?: (sectionId: string) => void;
}

function LLMEngineConfig({ config, onNavigate }: EngineConfigProps) {
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

function CaiyunEngineConfig({
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

function DeepLEngineConfig({
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

function BaiduEngineConfig({
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

function DeepLXEngineConfig({
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

function OfflineEngineConfig({ config, updateConfig, saveConfig }: EngineConfigProps) {
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
        <span className="ui-body text-text-secondary">
          {t('settings.enginePage.autoOffline')}
        </span>
      </label>
    </div>
  );
}

// Ordered engine row — ▲▼ reorder (HTML5 drag is unreliable in WebView)
interface SortableEngineCardProps {
  engineId: string;
  engineConfig: EngineDisplayConfig;
  config: AppConfig;
  updateConfig: ConfigUpdater;
  saveConfig: () => Promise<void>;
  showSecrets: Record<string, boolean>;
  toggleSecret: (key: string) => void;
  index: number;
  total: number;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onNavigate?: (sectionId: string) => void;
}

function SortableEngineCard({
  engineId,
  engineConfig,
  config,
  updateConfig,
  saveConfig,
  showSecrets,
  toggleSecret,
  index,
  total,
  onMoveUp,
  onMoveDown,
  onNavigate,
}: SortableEngineCardProps) {
  const { t } = useI18n();
  const getToggleHandler = () => {
    switch (engineId) {
      case 'google':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, google: { ...prev.engines.google, enabled } },
          }));
          void saveConfig();
        };
      case 'youdao':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, youdao: { ...prev.engines.youdao, enabled } },
          }));
          void saveConfig();
        };
      case 'caiyun':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              caiyun: {
                ...prev.engines.caiyun,
                enabled,
                apiToken: prev.engines.caiyun?.apiToken || '',
              },
            },
          }));
          void saveConfig();
        };
      case 'deepl':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              deepl: {
                ...prev.engines.deepl,
                enabled,
                apiKey: prev.engines.deepl.apiKey || '',
                pro: prev.engines.deepl.pro || false,
              },
            },
          }));
          void saveConfig();
        };
      case 'deeplx':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, deeplx: { ...prev.engines.deeplx, enabled } },
          }));
          void saveConfig();
        };
      case 'baidu':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, baidu: { ...prev.engines.baidu, enabled } },
          }));
          void saveConfig();
        };
      case 'microsoft':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, microsoft: { ...prev.engines.microsoft, enabled } },
          }));
          void saveConfig();
        };
      case 'yandex':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: { ...prev.engines, yandex: { ...prev.engines.yandex, enabled } },
          }));
          void saveConfig();
        };
      case 'offline':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              offline: {
                enabled,
                autoSwitch: prev.engines.offline.autoSwitch ?? true,
                downloadedModels: prev.engines.offline.downloadedModels ?? [],
                modelDir: prev.engines.offline.modelDir ?? '',
              },
            },
          }));
          void saveConfig();
        };
      case 'tatoeba':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              tatoeba: { ...(prev.engines.tatoeba || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      case 'baidu_web':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              baiduWeb: { ...(prev.engines.baiduWeb || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      case 'caiyun_web':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              caiyunWeb: { ...(prev.engines.caiyunWeb || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      case 'volcengine_web':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              volcengineWeb: { ...(prev.engines.volcengineWeb || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      case 'transmart':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              transmart: { ...(prev.engines.transmart || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      case 'papago':
        return (enabled: boolean) => {
          updateConfig((prev) => ({
            ...prev,
            engines: {
              ...prev.engines,
              papago: { ...(prev.engines.papago || { enabled: false }), enabled },
            },
          }));
          void saveConfig();
        };
      default:
        // eslint-disable-next-line @typescript-eslint/no-empty-function
        return () => {};
    }
  };

  return (
    <div className="flex items-stretch gap-1.5">
      <div className="flex flex-col items-center justify-center gap-0.5 shrink-0 pt-1">
        <span className="text-[10px] font-mono text-text-secondary w-5 text-center">
          {index + 1}
        </span>
        <button
          type="button"
          onClick={onMoveUp}
          disabled={index === 0}
          className="p-1 rounded-md text-text-secondary hover:text-text-primary hover:bg-bg-tertiary disabled:opacity-25 disabled:pointer-events-none"
          title={t('settings.enginePage.moveUp')}
        >
          <ChevronUp size={16} />
        </button>
        <button
          type="button"
          onClick={onMoveDown}
          disabled={index >= total - 1}
          className="p-1 rounded-md text-text-secondary hover:text-text-primary hover:bg-bg-tertiary disabled:opacity-25 disabled:pointer-events-none"
          title={t('settings.enginePage.moveDown')}
        >
          <ChevronDown size={16} />
        </button>
      </div>
      <div className="flex-1 min-w-0">
        <EngineCard
          name={engineConfig.name}
          enabled={engineConfig.enabled}
          onToggle={getToggleHandler()}
          status={engineConfig.status}
          badges={engineConfig.badges}
          description={engineConfig.description}
          hideToggle={engineId === 'llm'}
          alwaysShowChildren={engineId === 'llm'}
        >
          {engineId === 'llm' && (
            <LLMEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              onNavigate={onNavigate}
            />
          )}
          {engineId === 'caiyun' && (
            <CaiyunEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'deepl' && (
            <DeepLEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'deeplx' && (
            <DeepLXEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'baidu' && (
            <BaiduEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'offline' && (
            <OfflineEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
            />
          )}
        </EngineCard>
      </div>
    </div>
  );
}

// 引擎卡片组件
interface EngineCardProps {
  name: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  status: 'connected' | 'warning' | 'error';
  badges: Array<{ label: string; variant: 'success' | 'warning' | 'error' | 'info' }>;
  description: string;
  children?: React.ReactNode;
  hideToggle?: boolean;
  alwaysShowChildren?: boolean;
}

function EngineCard({
  name,
  enabled,
  onToggle,
  status,
  badges,
  description,
  children,
  hideToggle,
  alwaysShowChildren,
}: EngineCardProps) {
  const statusIcons = {
    connected: <CheckCircle size={15} className="text-text-secondary" />,
    warning: <AlertCircle size={15} className="text-text-secondary" />,
    error: <AlertCircle size={15} className="text-text-primary" />,
  };

  const Icon = name.includes('LLM') || name.includes('大模型') ? Bot : Languages;

  return (
    <div className="p-3.5 border border-border rounded-xl bg-bg-secondary">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3 flex-1 min-w-0">
          <div className="w-9 h-9 rounded-lg bg-bg-tertiary border border-border flex items-center justify-center shrink-0 text-text-secondary">
            <Icon size={18} strokeWidth={1.75} />
          </div>

          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-0.5">
              <h4 className="text-sm font-medium tracking-tight text-text-primary">{name}</h4>
              {statusIcons[status]}
            </div>
            <p className="text-xs text-text-secondary mb-2 leading-relaxed">{description}</p>
            <div className="flex flex-wrap gap-1.5">
              {badges.map((badge, idx) => (
                <Badge key={idx} variant={badge.variant}>
                  {badge.label}
                </Badge>
              ))}
            </div>
          </div>
        </div>

        {!hideToggle && <Switch checked={enabled} onChange={onToggle} />}
      </div>

      {(alwaysShowChildren || enabled) && children && (
        <div className="mt-3 pt-3 border-t border-border">{children}</div>
      )}
    </div>
  );
}
