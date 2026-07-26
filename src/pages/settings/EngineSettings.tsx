import { useState, useCallback } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { RoutingStrategy, AppConfig } from '../../types';
import { ROUTING_STRATEGIES } from './engines/routingStrategies';
import { isLlmConfigured } from './engines/enginesMeta';

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
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';
import {
  AlertCircle,
  CheckCircle,
  ExternalLink,
  Eye,
  EyeOff,
  Globe,
  Zap,
  Cloud,
  Bot,
  Database,
  HardDrive,
  GripVertical,
} from 'lucide-react';

export default function EngineSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const [showSecrets, setShowSecrets] = useState<Record<string, boolean>>({});
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  const toggleSecret = (key: string) => {
    setShowSecrets((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  // All possible engine IDs (default priority order)
  const allEngineIds = [
    'llm',
    'youdao',
    'caiyun',
    'deepl',
    'deeplx',
    'baidu',
    'microsoft',
    'yandex',
    'google',
    'offline',
  ];

  // Get current engine order (fall back to default if not configured)
  const engineOrder =
    config.engineOrder && config.engineOrder.length > 0 ? config.engineOrder : allEngineIds;

  // Save reordered engine list to config
  const persistEngineOrder = useCallback(
    (newOrder: string[]) => {
      updateConfig((prev) => ({ ...prev, engineOrder: newOrder }));
      void saveConfig();
    },
    [updateConfig, saveConfig],
  );

  // Drag handlers
  const handleDragStart = (e: React.DragEvent, idx: number) => {
    setDragIndex(idx);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', String(idx));
  };

  const handleDragOver = (e: React.DragEvent, idx: number) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    setDragOverIndex(idx);
  };

  const handleDragLeave = () => {
    setDragOverIndex(null);
  };

  const handleDrop = (e: React.DragEvent, dropIdx: number) => {
    e.preventDefault();
    if (dragIndex === null || dragIndex === dropIdx) {
      setDragIndex(null);
      setDragOverIndex(null);
      return;
    }
    const newOrder = [...engineOrder];
    const [moved] = newOrder.splice(dragIndex, 1);
    newOrder.splice(dropIdx, 0, moved);
    persistEngineOrder(newOrder);
    setDragIndex(null);
    setDragOverIndex(null);
  };

  const handleDragEnd = () => {
    setDragIndex(null);
    setDragOverIndex(null);
  };

  const currentStrategy = config.routingStrategy || 'fallback_on_error';

  // 引擎配置映射 - 根据ID返回引擎配置
  const getEngineConfig = (engineId: string): EngineDisplayConfig | null => {
    switch (engineId) {
      case 'llm': {
        const llmOk = isLlmConfigured(config.llm);
        return {
          id: 'llm',
          name: 'LLM 大模型翻译',
          enabled: llmOk,
          status: (llmOk ? 'connected' : 'warning') as 'connected' | 'warning' | 'error',
          badges: [
            { label: '主引擎', variant: 'info' as const },
            llmOk
              ? { label: '已配置', variant: 'success' as const }
              : { label: '需要 API Key', variant: 'warning' as const },
          ],
          description: '使用大语言模型进行翻译，支持上下文理解和自定义提示词',
        };
      }
      case 'google':
        return {
          id: 'google',
          name: 'Google Translation',
          enabled: config.engines.google.enabled,
          status: 'connected' as const,
          badges: [
            { label: '免费', variant: 'success' as const },
            { label: '无需配置', variant: 'info' as const },
          ],
          description: 'Google 提供的免费翻译服务，支持100+语言',
        };
      case 'youdao':
        return {
          id: 'youdao',
          name: '有道翻译',
          enabled: config.engines.youdao.enabled,
          status: 'connected' as const,
          badges: [{ label: '免费', variant: 'success' as const }],
          description: '有道提供的翻译服务',
        };
      case 'caiyun': {
        const hasToken = !!config.engines.caiyun?.apiToken.trim();
        return {
          id: 'caiyun',
          name: '彩云小译',
          enabled: config.engines.caiyun?.enabled || false,
          status: hasToken ? 'connected' : 'warning',
          badges: [
            { label: '免费额度', variant: 'success' as const },
            hasToken
              ? { label: '已配置', variant: 'success' as const }
              : { label: '需填写密钥后才会被路由使用', variant: 'warning' as const },
          ],
          description: '擅长长文本和小说翻译，免费额度100万字/月',
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
            { label: '付费', variant: 'warning' as const },
            hasKey
              ? { label: '已配置', variant: 'success' as const }
              : { label: '需填写密钥后才会被路由使用', variant: 'warning' as const },
          ],
          description: '高质量机器翻译服务',
        };
      }
      case 'deeplx':
        return {
          id: 'deeplx',
          name: 'DeepLX',
          enabled: config.engines.deeplx.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: '免费', variant: 'success' as const },
            { label: '可选自建 Key', variant: 'info' as const },
          ],
          description: '免费的 DeepL 翻译接口，可选自建 Key',
        };
      case 'baidu': {
        const hasAppId = !!config.engines.baidu.appId.trim();
        return {
          id: 'baidu',
          name: '百度翻译',
          enabled: config.engines.baidu.enabled,
          status: hasAppId ? 'connected' : 'warning',
          badges: [
            { label: '免费额度', variant: 'success' as const },
            hasAppId
              ? { label: '已配置', variant: 'success' as const }
              : { label: '需填写密钥后才会被路由使用', variant: 'warning' as const },
          ],
          description: '百度提供的翻译服务，支持200+语言',
        };
      }
      case 'microsoft':
        return {
          id: 'microsoft',
          name: '微软翻译',
          enabled: config.engines.microsoft.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: '免费', variant: 'success' as const },
            { label: '无需配置', variant: 'info' as const },
          ],
          description: '微软提供的翻译服务',
        };
      case 'yandex':
        return {
          id: 'yandex',
          name: 'Yandex翻译',
          enabled: config.engines.yandex.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: '免费', variant: 'success' as const },
            { label: '无需配置', variant: 'info' as const },
          ],
          description: 'Yandex 提供的翻译服务',
        };
      case 'offline':
        return {
          id: 'offline',
          name: '离线翻译',
          enabled: config.engines.offline.enabled || false,
          status: 'warning' as const,
          badges: [
            { label: '本地', variant: 'info' as const },
            { label: '需下载模型', variant: 'warning' as const },
          ],
          description: '完全本地化的翻译模型，无需网络连接',
        };
      default:
        return null;
    }
  };

  return (
    <div className="space-y-5">
      {/* 页面标题 */}
      <div>
        <h1 className="text-xl font-semibold text-text-primary">翻译引擎设置</h1>
        <p className="text-xs text-text-secondary mt-1">配置翻译引擎和路由策略</p>
      </div>

      {/* 路由策略选择 */}
      <Card title="路由策略" description="选择如何使用配置的翻译引擎">
        <div className="space-y-3">
          {ROUTING_STRATEGIES.map((strategy) => (
            <label
              key={strategy.id}
              className={`flex items-start gap-3 p-4 rounded-lg border-2 cursor-pointer transition-all ${
                currentStrategy === strategy.id
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-border/60'
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
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-text-primary">{strategy.label}</span>
                  {strategy.recommended && <Badge variant="info">推荐</Badge>}
                </div>
                <p className="text-sm text-text-secondary mt-1">{strategy.description}</p>
              </div>
            </label>
          ))}
        </div>

        {/* 当前使用的引擎提示 */}
        <div className="mt-4 p-3 bg-white/10 border border-border rounded-lg">
          <div className="flex items-start gap-2">
            <AlertCircle size={16} className="text-primary dark:text-primary mt-0.5 shrink-0" />
            <div className="text-sm">
              <p className="font-medium text-primary dark:text-primary">
                当前策略：{ROUTING_STRATEGIES.find((s) => s.id === currentStrategy)?.label}
              </p>
              <p className="text-text-secondary mt-1">
                {currentStrategy === 'fallback_on_error' &&
                  '翻译时会按下方引擎顺序依次尝试，第一个成功的结果会被返回'}
                {currentStrategy === 'parallel_compare' &&
                  '翻译时会同时调用所有已启用的引擎，所有结果都会显示'}
                {currentStrategy === 'cost_aware' &&
                  '翻译时会优先使用免费引擎（Google、Youdao等），失败后才尝试付费引擎'}
              </p>
            </div>
          </div>
        </div>
      </Card>

      {/* 引擎配置列表 */}
      <Card title="引擎配置" description="拖拽左侧手柄调整引擎顺序，开关控制启用">
        <div className="space-y-3">
          {engineOrder.map((engineId, idx) => {
            const engineConfig = getEngineConfig(engineId);
            if (!engineConfig) return null;

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
                isDragging={dragIndex === idx}
                isDragOver={dragOverIndex === idx}
                onDragStart={(e) => handleDragStart(e, idx)}
                onDragOver={(e) => handleDragOver(e, idx)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, idx)}
                onDragEnd={handleDragEnd}
              />
            );
          })}
        </div>
        {engineOrder.length > 0 && (
          <p className="text-xs text-text-secondary mt-3 flex items-center gap-1">
            <GripVertical size={12} />
            拖拽左侧手柄可调整引擎优先级顺序
          </p>
        )}
      </Card>
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
}

function LLMEngineConfig({ config }: EngineConfigProps) {
  const configured = isLlmConfigured(config.llm);
  const model = config.llm.model.trim() || '未设置模型';
  const provider = config.llm.provider || 'custom';
  return (
    <div className="mt-3 space-y-2">
      <p className="text-sm text-text-secondary">
        状态：{configured ? `已配置（${provider} / ${model}）` : '未配置 API Key'}
      </p>
      <p className="text-sm text-primary">在「AI 增强」中配置</p>
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
            placeholder="输入彩云小译 API Token"
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
        获取 API Token <ExternalLink size={12} />
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
            placeholder="输入 DeepL API Key"
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
        <span className="text-sm text-text-secondary">Pro 账户</span>
      </label>
      <a
        href="https://www.deepl.com/pro-api"
        target="_blank"
        rel="noopener noreferrer"
        className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
      >
        获取 API Key <ExternalLink size={12} />
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
          placeholder="输入百度翻译 APP ID"
          className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
        />
      </div>
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">密钥</label>
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
            placeholder="输入百度翻译密钥"
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
        获取 API 凭证 <ExternalLink size={12} />
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
  return (
    <div className="mt-3 space-y-3">
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">API Key（可选）</label>
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
            placeholder="可选，自建 DeepLX 服务 Key"
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
  return (
    <div className="mt-3 space-y-2">
      <p className="text-xs text-text-secondary">
        已下载模型: {config.engines.offline.downloadedModels.length || 0} 个
      </p>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={config.engines.offline.autoSwitch || false}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                offline: { ...prev.engines.offline, autoSwitch: e.target.checked },
              },
            }));
            void saveConfig();
          }}
          className="rounded"
        />
        <span className="text-sm text-text-secondary">离线可用时自动切换</span>
      </label>
    </div>
  );
}

// 可拖拽排序的引擎卡片包装器
interface SortableEngineCardProps {
  engineId: string;
  engineConfig: EngineDisplayConfig;
  config: AppConfig;
  updateConfig: ConfigUpdater;
  saveConfig: () => Promise<void>;
  showSecrets: Record<string, boolean>;
  toggleSecret: (key: string) => void;
  index: number;
  isDragging: boolean;
  isDragOver: boolean;
  onDragStart: (e: React.DragEvent) => void;
  onDragOver: (e: React.DragEvent) => void;
  onDragLeave: () => void;
  onDrop: (e: React.DragEvent) => void;
  onDragEnd: () => void;
}

function SortableEngineCard({
  engineId,
  engineConfig,
  config,
  updateConfig,
  saveConfig,
  showSecrets,
  toggleSecret,
  isDragging,
  isDragOver,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
  onDragEnd,
}: SortableEngineCardProps) {
  // 根据不同引擎渲染不同的toggle和配置
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
            engines: { ...prev.engines, offline: { ...prev.engines.offline, enabled } },
          }));
          void saveConfig();
        };
      default:
        // eslint-disable-next-line @typescript-eslint/no-empty-function
        return () => {};
    }
  };

  return (
    <div
      draggable
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onDragEnd={onDragEnd}
      className={`flex items-start gap-1 transition-all ${
        isDragging ? 'opacity-50 scale-[0.98]' : ''
      } ${isDragOver ? 'ring-2 ring-primary ring-inset rounded-lg' : ''}`}
    >
      {/* Drag handle */}
      <div
        className="mt-4 pt-1 cursor-grab active:cursor-grabbing text-text-secondary hover:text-text-primary shrink-0"
        title="拖拽排序"
      >
        <GripVertical size={16} />
      </div>
      <div className="flex-1 min-w-0">
        <EngineCard
          name={engineConfig.name}
          icon=""
          enabled={engineConfig.enabled}
          onToggle={getToggleHandler()}
          status={engineConfig.status}
          badges={engineConfig.badges}
          description={engineConfig.description}
          hideToggle={engineId === 'llm'}
          alwaysShowChildren={engineId === 'llm'}
        >
          {engineId === 'llm' && (
            <LLMEngineConfig config={config} updateConfig={updateConfig} saveConfig={saveConfig} />
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
  icon: string;
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
    connected: <CheckCircle size={16} className="text-green-600 dark:text-green-400" />,
    warning: <AlertCircle size={16} className="text-yellow-600 dark:text-yellow-400" />,
    error: <AlertCircle size={16} className="text-red-600 dark:text-red-400" />,
  };

  // 引擎图标映射 - 使用 lucide-react 图标
  const getEngineIcon = () => {
    if (name.includes('LLM') || name.includes('大模型')) {
      return <Bot size={20} className="text-primary" />;
    }
    if (name.includes('Google')) {
      return <Globe size={20} className="text-primary" />;
    }
    if (name.includes('有道')) {
      return <Globe size={20} className="text-red-500" />;
    }
    if (name.includes('彩云')) {
      return <Cloud size={20} className="text-primary" />;
    }
    if (name.includes('DeepL')) {
      return <Zap size={20} className="text-primary" />;
    }
    if (name.includes('百度')) {
      return <Globe size={20} className="text-primary" />;
    }
    if (name.includes('微软')) {
      return <Globe size={20} className="text-neutral-500" />;
    }
    if (name.includes('Yandex')) {
      return <Globe size={20} className="text-red-600" />;
    }
    if (name.includes('离线')) {
      return <HardDrive size={20} className="text-gray-500" />;
    }
    return <Database size={20} className="text-gray-400" />;
  };

  return (
    <div className="p-4 border border-border rounded-lg bg-bg-primary">
      <div className="flex items-start justify-between">
        <div className="flex items-start gap-3 flex-1">
          {/* 引擎图标 */}
          <div className="w-10 h-10 rounded-lg bg-bg-secondary flex items-center justify-center shrink-0">
            {getEngineIcon()}
          </div>

          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <h4 className="font-medium text-text-primary">{name}</h4>
              {statusIcons[status]}
            </div>
            <p className="text-sm text-text-secondary mb-2">{description}</p>
            <div className="flex flex-wrap gap-2">
              {badges.map((badge, idx) => (
                <Badge key={idx} variant={badge.variant}>
                  {badge.label}
                </Badge>
              ))}
            </div>
          </div>
        </div>

        {/* 开关（LLM 由凭证决定，无假开关） */}
        {!hideToggle && <Switch checked={enabled} onChange={onToggle} />}
      </div>

      {/* 额外配置 */}
      {(alwaysShowChildren || enabled) && children && (
        <div className="mt-4 pt-4 border-t border-border">{children}</div>
      )}
    </div>
  );
}
