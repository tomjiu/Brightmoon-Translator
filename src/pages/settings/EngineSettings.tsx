import { useState, useCallback, useMemo } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { RoutingStrategy, AppConfig } from '../../types';
import { ROUTING_STRATEGIES } from './engines/routingStrategies';
import {
  DEFAULT_ENGINE_ORDER,
  ENGINE_SECTIONS,
  getEngineSection,
  isLlmConfigured,
} from './engines/enginesMeta';
import { useReorderDrag } from './engines/useReorderDrag';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import LLMEngineConfig from './engines/LLMEngineConfig';
import CaiyunEngineConfig from './engines/CaiyunEngineConfig';
import DeepLEngineConfig from './engines/DeepLEngineConfig';
import BaiduEngineConfig from './engines/BaiduEngineConfig';
import DeepLXEngineConfig from './engines/DeepLXEngineConfig';
import OfflineEngineConfig from './engines/OfflineEngineConfig';
import EngineCard from './engines/EngineCard';
import { ExternalLink, GripVertical } from 'lucide-react';
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
    // P3 fix: filter invalid IDs + dedupe. filter(Boolean) only drops empty
    // strings; it keeps typos like 'deepl_web' or 'Google' (wrong case) that
    // cause position-number gaps in the UI.
    const validIds = new Set<string>(DEFAULT_ENGINE_ORDER);
    const saved = (config.engineOrder ?? []).filter(
      (id): id is string => typeof id === 'string' && validIds.has(id),
    );
    // Dedupe (first occurrence wins to preserve user order)
    const deduped: string[] = [];
    const seen = new Set<string>();
    for (const id of saved) {
      if (!seen.has(id)) {
        deduped.push(id);
        seen.add(id);
      }
    }
    const base = deduped.length > 0 ? deduped.slice() : [];
    // 仅在已有已保存顺序时才补缺失引擎;否则直接返回默认顺序(避免把默认表拼两份,导致每个引擎重复)
    if (deduped.length === 0) return [...DEFAULT_ENGINE_ORDER];
    const seenSaved = new Set(deduped);
    const missing = DEFAULT_ENGINE_ORDER.filter((id) => !seenSaved.has(id));
    return [...base, ...missing];
  }, [config.engineOrder]);

  const persistEngineOrder = useCallback(
    (newOrder: string[]) => {
      updateConfig((prev) => ({ ...prev, engineOrder: newOrder }));
      void saveConfig();
    },
    [updateConfig, saveConfig],
  );

  // 引擎配置映射 - 根据ID返回引擎配置
  const getEngineConfig = useCallback(
    (engineId: string): EngineDisplayConfig | null => {
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
    },
    [config, t],
  );

  const rawStrategy = config.routingStrategy || 'fallback_on_error';
  const currentStrategy = ROUTING_STRATEGIES.some((s) => s.id === rawStrategy)
    ? rawStrategy
    : 'fallback_on_error';

  // 已启用引擎：按 engineOrder 顺序（即路由/OCR 回退优先级），LLM 永远在内
  const enabledIds = useMemo(
    () =>
      engineOrder.filter((id) => id === 'llm' || !!getEngineConfig(id)?.enabled),
    [engineOrder, getEngineConfig],
  );
  const enabledSet = useMemo(() => new Set(enabledIds), [enabledIds]);

  // 未启用引擎：按分类（llm/official/web/offline）默认顺序分组
  const disabledBySection = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const section of ENGINE_SECTIONS) {
      map.set(
        section.id,
        DEFAULT_ENGINE_ORDER.filter(
          (id) => !enabledSet.has(id) && getEngineSection(id) === section.id,
        ),
      );
    }
    return map;
  }, [enabledSet]);

  // 拖拽提交：新启用顺序在前，其余按原 engineOrder 相对顺序收尾
  const handleDragCommit = useCallback(
    (newEnabledOrder: string[]) => {
      const rest = engineOrder.filter((id) => !enabledSet.has(id));
      persistEngineOrder([...newEnabledOrder, ...rest]);
    },
    [engineOrder, enabledSet, persistEngineOrder],
  );

  const { order: enabledOrder, isDragging, dragHandleProps } = useReorderDrag(
    enabledIds,
    handleDragCommit,
  );

  // 启用一个引擎时移到优先级列表末尾（追加到已启用顺序的队尾）
  const bumpToEnd = useCallback(
    (engineId: string) => {
      persistEngineOrder([...engineOrder.filter((id) => id !== engineId), engineId]);
    },
    [engineOrder, persistEngineOrder],
  );

  const engineRowProps = {
    config,
    updateConfig,
    saveConfig,
    showSecrets,
    toggleSecret,
    onNavigate,
    onEnable: bumpToEnd,
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

      <Card
        title={t('settings.enginePage.enabledTitle')}
        description={t('settings.enginePage.enabledDesc')}
      >
        <div className="space-y-2">
          {enabledOrder.map((engineId, idx) => {
            const engineConfig = getEngineConfig(engineId);
            if (!engineConfig) return null;
            return (
              <EngineRow
                key={engineId}
                {...engineRowProps}
                engineId={engineId}
                engineConfig={engineConfig}
                index={idx}
                dragging={isDragging(engineId)}
                dragHandleProps={dragHandleProps(engineId)}
              />
            );
          })}
          {enabledOrder.length === 0 && (
            <p className="text-xs text-text-secondary">{t('settings.enginePage.noEnabled')}</p>
          )}
        </div>
      </Card>

      <Card
        title={t('settings.enginePage.disabledTitle')}
        description={t('settings.enginePage.disabledDesc')}
      >
        <div className="space-y-5">
          {ENGINE_SECTIONS.map((section) => {
            const sectionIds = disabledBySection.get(section.id) ?? [];
            if (sectionIds.length === 0) return null;
            return (
              <div key={section.id}>
                <h3 className="text-xs font-medium uppercase tracking-wide text-text-secondary mb-2">
                  {t(section.title)}
                </h3>
                <div className="space-y-2">
                  {sectionIds.map((engineId) => {
                    const engineConfig = getEngineConfig(engineId);
                    if (!engineConfig) return null;
                    return (
                      <EngineRow
                        key={engineId}
                        {...engineRowProps}
                        engineId={engineId}
                        engineConfig={engineConfig}
                      />
                    );
                  })}
                </div>
              </div>
            );
          })}
          {[...disabledBySection.values()].every((ids) => ids.length === 0) && (
            <p className="text-xs text-text-secondary">{t('settings.enginePage.noDisabled')}</p>
          )}
        </div>
      </Card>
    </div>
  );
}

// Ordered engine row — draggable handle when enabled, plain card when not
interface EngineRowProps {
  engineId: string;
  engineConfig: EngineDisplayConfig;
  config: AppConfig;
  updateConfig: ConfigUpdater;
  saveConfig: () => Promise<void>;
  showSecrets: Record<string, boolean>;
  toggleSecret: (key: string) => void;
  onEnable: (engineId: string) => void;
  onNavigate?: (sectionId: string) => void;
  index?: number;
  dragging?: boolean;
  dragHandleProps?: {
    'data-engine-drag-id': string;
    onPointerDown: (e: React.PointerEvent<HTMLElement>) => void;
  };
}

function EngineRow({
  engineId,
  engineConfig,
  config,
  updateConfig,
  saveConfig,
  showSecrets,
  toggleSecret,
  onEnable,
  onNavigate,
  index,
  dragging = false,
  dragHandleProps,
}: EngineRowProps) {
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

  const engineToggle = getToggleHandler();
  const handleToggle = (enabled: boolean) => {
    engineToggle(enabled);
    if (enabled) onEnable(engineId);
  };

  return (
    <div
      data-engine-drag-id={engineId}
      className={`flex items-stretch gap-1.5 rounded-xl ${
        dragging ? 'opacity-60 ring-2 ring-primary ring-offset-1' : ''
      }`}
    >
      {dragHandleProps && typeof index === 'number' && (
        <div
          {...dragHandleProps}
          title={t('settings.enginePage.dragHandle')}
          className={`flex flex-col items-center justify-center gap-0.5 shrink-0 pt-1 px-1 rounded-lg select-none touch-none ${
            dragging ? 'cursor-grabbing' : 'cursor-grab hover:bg-bg-tertiary'
          }`}
        >
          <span className="text-[10px] font-mono text-text-secondary w-5 text-center">
            {index + 1}
          </span>
          <GripVertical size={16} className="text-text-secondary" />
        </div>
      )}
      <div className="flex-1 min-w-0">
        <EngineCard
          name={engineConfig.name}
          enabled={engineConfig.enabled}
          onToggle={handleToggle}
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

