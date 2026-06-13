import { useState, useCallback } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { RoutingStrategy, AppConfig } from '../../types';

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

  // 路由策略说明
  const routingStrategies = [
    {
      id: 'fallback_on_error',
      label: '回退模式',
      description: '按引擎顺序尝试，第一个成功就返回（推荐）',
      recommended: true,
    },
    {
      id: 'parallel_compare',
      label: '并行模式',
      description: '同时调用多个引擎，显示所有结果',
    },
    {
      id: 'cost_aware',
      label: '成本优先',
      description: '优先使用免费引擎，失败后尝试付费引擎',
    },
  ];

  const currentStrategy = config.routingStrategy || 'fallback_on_error';

  // 引擎配置映射 - 根据ID返回引擎配置
  const getEngineConfig = (engineId: string): EngineDisplayConfig | null => {
    switch (engineId) {
      case 'llm':
        return {
          id: 'llm',
          name: 'LLM 大模型翻译',
          enabled: !!config.llm.apiKey,
          status: (config.llm.apiKey ? 'connected' : 'warning') as
            | 'connected'
            | 'warning'
            | 'error',
          badges: [
            { label: '主引擎', variant: 'info' as const },
            { label: '高质量', variant: 'success' as const },
          ],
          description: '使用大语言模型进行翻译，支持上下文理解和自定义提示词',
        };
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
      case 'caiyun':
        return {
          id: 'caiyun',
          name: '彩云小译',
          enabled: config.engines.caiyun?.enabled || false,
          status: config.engines.caiyun?.apiToken ? 'connected' : 'warning',
          badges: [
            { label: '免费额度', variant: 'success' as const },
            { label: '需要配置', variant: 'warning' as const },
          ],
          description: '擅长长文本和小说翻译，免费额度100万字/月',
        };
      case 'deepl':
        return {
          id: 'deepl',
          name: 'DeepL',
          enabled: config.engines.deepl.enabled || false,
          status: config.engines.deepl.apiKey ? 'connected' : 'warning',
          badges: [
            { label: '付费', variant: 'warning' as const },
            { label: '需要API Key', variant: 'warning' as const },
          ],
          description: '高质量机器翻译服务',
        };
      case 'deeplx':
        return {
          id: 'deeplx',
          name: 'DeepLX',
          enabled: config.engines.deeplx.enabled || false,
          status: 'connected' as const,
          badges: [
            { label: '免费', variant: 'success' as const },
            { label: '内置', variant: 'info' as const },
          ],
          description: '免费的 DeepL 翻译接口，无需 API Key',
        };
      case 'baidu':
        return {
          id: 'baidu',
          name: '百度翻译',
          enabled: config.engines.baidu.enabled,
          status: config.engines.baidu.appId ? 'connected' : 'warning',
          badges: [
            { label: '免费额度', variant: 'success' as const },
            { label: '需要配置', variant: 'warning' as const },
          ],
          description: '百度提供的翻译服务，支持200+语言',
        };
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
          {routingStrategies.map((strategy) => (
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
        <div className="mt-4 p-3 bg-blue-500/10 border border-blue-500/30 rounded-lg">
          <div className="flex items-start gap-2">
            <AlertCircle size={16} className="text-blue-600 dark:text-blue-400 mt-0.5 shrink-0" />
            <div className="text-sm">
              <p className="font-medium text-blue-600 dark:text-blue-400">
                当前策略：{routingStrategies.find((s) => s.id === currentStrategy)?.label}
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

function LLMEngineConfig({
  config,
  updateConfig,
  saveConfig,
  showSecrets,
  toggleSecret,
}: EngineConfigProps) {
  const [newTemplateName, setNewTemplateName] = useState('');

  const saveTemplate = () => {
    if (!newTemplateName.trim() || !config.customPrompt.trim()) return;
    const templates = config.promptTemplates || [];
    const existing = templates.findIndex((t) => t.name === newTemplateName.trim());
    if (existing >= 0) {
      // Update existing template
      const updated = [...templates];
      updated[existing] = { ...updated[existing], prompt: config.customPrompt };
      updateConfig((prev) => ({ ...prev, promptTemplates: updated }));
    } else {
      updateConfig((prev) => ({
        ...prev,
        promptTemplates: [
          ...(prev.promptTemplates || []),
          { name: newTemplateName.trim(), prompt: config.customPrompt },
        ],
      }));
    }
    void saveConfig();
    setNewTemplateName('');
  };

  const applyTemplate = (prompt: string) => {
    updateConfig((prev) => ({ ...prev, customPrompt: prompt }));
    void saveConfig();
  };

  const deleteTemplate = (index: number) => {
    const templates = config.promptTemplates || [];
    updateConfig((prev) => ({
      ...prev,
      promptTemplates: templates.filter((_, i) => i !== index),
    }));
    void saveConfig();
  };

  return (
    <div className="mt-3 space-y-4">
      {/* API Key */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
        <div className="relative">
          <input
            type={showSecrets?.llm ? 'text' : 'password'}
            value={config.llm.apiKey || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                llm: { ...prev.llm, apiKey: e.target.value },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder="输入 API Key"
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none pr-10"
          />
          <button
            onClick={() => toggleSecret?.('llm')}
            className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
          >
            {showSecrets?.llm ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
        </div>
      </div>

      {/* Backup API Keys */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">
          备用 API Keys（故障切换）
        </label>
        <div className="space-y-2">
          {(config.llm.apiKeys || []).map((key: string, idx: number) => (
            <div key={idx} className="flex gap-2">
              <input
                type="text"
                value={key}
                onChange={(e) => {
                  const keys = [...(config.llm.apiKeys || [])];
                  keys[idx] = e.target.value;
                  updateConfig((prev) => ({ ...prev, llm: { ...prev.llm, apiKeys: keys } }));
                }}
                onBlur={() => void saveConfig()}
                placeholder={`备用 Key ${idx + 1}`}
                className="flex-1 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none text-sm"
              />
              <button
                className="px-2 text-error hover:bg-error/10 rounded transition-colors"
                onClick={() => {
                  const keys = (config.llm.apiKeys || []).filter(
                    (_: string, i: number) => i !== idx,
                  );
                  updateConfig((prev) => ({ ...prev, llm: { ...prev.llm, apiKeys: keys } }));
                  void saveConfig();
                }}
              >
                ✕
              </button>
            </div>
          ))}
          <button
            className="text-sm text-primary hover:underline"
            onClick={() => {
              const keys = [...(config.llm.apiKeys || []), ''];
              updateConfig((prev) => ({ ...prev, llm: { ...prev.llm, apiKeys: keys } }));
            }}
          >
            + 添加备用 Key
          </button>
        </div>
      </div>

      {/* Base URL & Model */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="block text-sm font-medium text-text-primary mb-2">Base URL</label>
          <input
            type="text"
            value={config.llm.baseUrl || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                llm: { ...prev.llm, baseUrl: e.target.value },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder="https://api.deepseek.com/v1"
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-text-primary mb-2">Model</label>
          <input
            type="text"
            value={config.llm.model || ''}
            onChange={(e) => {
              updateConfig((prev) => ({
                ...prev,
                llm: { ...prev.llm, model: e.target.value },
              }));
            }}
            onBlur={() => void saveConfig()}
            placeholder="deepseek-chat"
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
          />
        </div>
      </div>

      {/* Provider selector */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">服务商</label>
        <select
          value={config.llm.provider || 'custom'}
          onChange={(e) => {
            const provider = e.target.value as AppConfig['llm']['provider'];
            const presets: Record<string, { baseUrl: string; model: string }> = {
              deepseek: { baseUrl: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
              openai: { baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o' },
            };
            const preset = presets[provider];
            updateConfig((prev) => ({
              ...prev,
              llm: {
                ...prev.llm,
                provider,
                ...(preset ? { baseUrl: preset.baseUrl, model: preset.model } : {}),
              },
            }));
            void saveConfig();
          }}
          className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none"
        >
          <option value="deepseek">DeepSeek</option>
          <option value="openai">OpenAI</option>
          <option value="custom">自定义</option>
        </select>
      </div>

      <p className="text-xs text-text-secondary">
        支持 DeepSeek、OpenAI、Claude 等兼容 OpenAI API 的模型
      </p>

      {/* ── Temperature ── */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">
          温度 (Temperature): {config.llmTemperature ?? 0.3}
        </label>
        <input
          type="range"
          min="0"
          max="2"
          step="0.1"
          value={config.llmTemperature ?? 0.3}
          onChange={(e) => {
            updateConfig((prev) => ({ ...prev, llmTemperature: parseFloat(e.target.value) }));
          }}
          onMouseUp={() => void saveConfig()}
          onTouchEnd={() => void saveConfig()}
          className="w-full accent-primary"
        />
        <div className="flex justify-between text-xs text-text-secondary">
          <span>精确 (0)</span>
          <span>创意 (2)</span>
        </div>
      </div>

      {/* ── Max Tokens ── */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">
          最大 Token: {config.llmMaxTokens ?? 4096}
        </label>
        <input
          type="range"
          min="256"
          max="16384"
          step="256"
          value={config.llmMaxTokens ?? 4096}
          onChange={(e) => {
            updateConfig((prev) => ({ ...prev, llmMaxTokens: parseInt(e.target.value, 10) }));
          }}
          onMouseUp={() => void saveConfig()}
          onTouchEnd={() => void saveConfig()}
          className="w-full accent-primary"
        />
        <div className="flex justify-between text-xs text-text-secondary">
          <span>256</span>
          <span>16384</span>
        </div>
      </div>

      {/* ── Custom Prompt ── */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">自定义翻译提示词</label>
        <p className="text-xs text-text-secondary mb-2">
          自定义 LLM 翻译的系统提示词。可用变量: {'{source_lang}'}, {'{target_lang}'}
        </p>
        <textarea
          value={config.customPrompt || ''}
          onChange={(e) => {
            updateConfig((prev) => ({ ...prev, customPrompt: e.target.value }));
          }}
          onBlur={() => void saveConfig()}
          placeholder="留空使用默认提示词。输入自定义提示词来控制翻译风格、格式等..."
          rows={4}
          className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none resize-y text-sm"
        />
      </div>

      {/* ── Prompt Templates ── */}
      <div>
        <label className="block text-sm font-medium text-text-primary mb-2">提示词模板</label>
        {config.promptTemplates && config.promptTemplates.length > 0 && (
          <div className="space-y-1 mb-3">
            {config.promptTemplates.map((tpl, idx) => (
              <div
                key={idx}
                className="flex items-center gap-2 bg-bg-secondary rounded-lg px-3 py-2"
              >
                <button
                  className="flex-1 text-left text-sm text-text-primary hover:text-primary truncate"
                  onClick={() => applyTemplate(tpl.prompt)}
                  title={tpl.prompt}
                >
                  {tpl.name}
                </button>
                <button
                  className="text-xs text-error hover:bg-error/10 px-1.5 py-0.5 rounded shrink-0"
                  onClick={() => deleteTemplate(idx)}
                >
                  删除
                </button>
              </div>
            ))}
          </div>
        )}
        <div className="flex gap-2">
          <input
            type="text"
            value={newTemplateName}
            onChange={(e) => setNewTemplateName(e.target.value)}
            placeholder="模板名称"
            className="flex-1 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none text-sm"
            onKeyDown={(e) => {
              if (e.key === 'Enter') saveTemplate();
            }}
          />
          <button
            className="px-3 py-2 bg-primary text-white rounded-lg hover:bg-primary/90 disabled:opacity-50 text-sm"
            onClick={saveTemplate}
            disabled={!newTemplateName.trim() || !config.customPrompt.trim()}
          >
            保存模板
          </button>
        </div>
        <p className="text-xs text-text-secondary mt-1">
          将当前提示词保存为模板，点击模板名称可快速应用
        </p>
      </div>
    </div>
  );
}

function YoudaoEngineConfig({ config, updateConfig, saveConfig }: EngineConfigProps) {
  return (
    <div className="mt-3 space-y-2">
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={config.engines.youdao.useAi || false}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                youdao: { ...prev.engines.youdao, useAi: e.target.checked },
              },
            }));
            void saveConfig();
          }}
          className="rounded"
        />
        <span className="text-sm text-text-secondary">使用AI增强翻译</span>
      </label>
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

function OfflineEngineConfig({ config }: { config: AppConfig }) {
  return (
    <div className="mt-3 space-y-2">
      <p className="text-xs text-text-secondary">
        已下载模型: {config.engines.offline.downloadedModels.length || 0} 个
      </p>
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
      case 'llm':
        // eslint-disable-next-line @typescript-eslint/no-empty-function
        return () => {}; // LLM通过API Key控制
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
        >
          {engineId === 'llm' && (
            <LLMEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'youdao' && (
            <YoudaoEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
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
          {engineId === 'baidu' && (
            <BaiduEngineConfig
              config={config}
              updateConfig={updateConfig}
              saveConfig={saveConfig}
              showSecrets={showSecrets}
              toggleSecret={toggleSecret}
            />
          )}
          {engineId === 'offline' && <OfflineEngineConfig config={config} />}
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
}

function EngineCard({
  name,
  enabled,
  onToggle,
  status,
  badges,
  description,
  children,
}: EngineCardProps) {
  const statusIcons = {
    connected: <CheckCircle size={16} className="text-green-600 dark:text-green-400" />,
    warning: <AlertCircle size={16} className="text-yellow-600 dark:text-yellow-400" />,
    error: <AlertCircle size={16} className="text-red-600 dark:text-red-400" />,
  };

  // 引擎图标映射 - 使用 lucide-react 图标
  const getEngineIcon = () => {
    if (name.includes('LLM') || name.includes('大模型')) {
      return <Bot size={20} className="text-purple-500" />;
    }
    if (name.includes('Google')) {
      return <Globe size={20} className="text-blue-500" />;
    }
    if (name.includes('有道')) {
      return <Globe size={20} className="text-red-500" />;
    }
    if (name.includes('彩云')) {
      return <Cloud size={20} className="text-sky-400" />;
    }
    if (name.includes('DeepL')) {
      return <Zap size={20} className="text-blue-600" />;
    }
    if (name.includes('百度')) {
      return <Globe size={20} className="text-blue-700" />;
    }
    if (name.includes('微软')) {
      return <Globe size={20} className="text-cyan-500" />;
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

        {/* 开关 */}
        <Switch checked={enabled} onChange={onToggle} />
      </div>

      {/* 额外配置 */}
      {enabled && children && <div className="mt-4 pt-4 border-t border-border">{children}</div>}
    </div>
  );
}
