import { useState } from 'react';
import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';
import { AlertCircle, CheckCircle, ExternalLink, Eye, EyeOff } from 'lucide-react';

export default function EngineSettings() {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  const [showSecrets, setShowSecrets] = useState<Record<string, boolean>>({});

  const toggleSecret = (key: string) => {
    setShowSecrets((prev) => ({ ...prev, [key]: !prev[key] }));
  };

  // 路由策略说明
  const routingStrategies = [
    {
      id: 'fallback',
      label: '回退模式',
      description: '按引擎顺序尝试，第一个成功就返回（推荐）',
      recommended: true,
    },
    {
      id: 'parallel',
      label: '并行模式',
      description: '同时调用多个引擎，显示所有结果',
    },
    {
      id: 'cost_aware',
      label: '成本优先',
      description: '优先使用免费引擎，失败后尝试付费引擎',
    },
  ];

  const currentStrategy = config.routingStrategy || 'fallback';

  return (
    <div className="space-y-6">
      {/* 页面标题 */}
      <div>
        <h1 className="text-2xl font-bold text-text-primary">翻译引擎设置</h1>
        <p className="text-sm text-text-secondary mt-1">配置翻译引擎和路由策略</p>
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
                    routingStrategy: e.target.value as 'fallback' | 'parallel' | 'cost_aware',
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
                {currentStrategy === 'fallback' &&
                  '翻译时会按下方引擎顺序依次尝试，第一个成功的结果会被返回'}
                {currentStrategy === 'parallel' &&
                  '翻译时会同时调用所有已启用的引擎，所有结果都会显示'}
                {currentStrategy === 'cost_aware' &&
                  '翻译时会优先使用免费引擎（Google、Youdao等），失败后才尝试付费引擎'}
              </p>
            </div>
          </div>
        </div>
      </Card>

      {/* 引擎配置列表 */}
      <Card title="引擎配置" description="启用和配置各个翻译引擎">
        <div className="space-y-4">
          {/* Google Translation */}
          <EngineCard
            name="Google Translation"
            icon="/icons/google.svg"
            enabled={config.engines.google.enabled}
            onToggle={(enabled) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  google: { ...prev.engines.google, enabled },
                },
              }));
              void saveConfig();
            }}
            status="connected"
            badges={[
              { label: '免费', variant: 'success' },
              { label: '无需配置', variant: 'info' },
            ]}
            description="Google 提供的免费翻译服务，支持100+语言"
          />

          {/* 有道翻译 */}
          <EngineCard
            name="有道翻译"
            icon="/icons/youdao.svg"
            enabled={config.engines.youdao.enabled}
            onToggle={(enabled) => {
              updateConfig((prev) => ({
                ...prev,
                engines: {
                  ...prev.engines,
                  youdao: { ...prev.engines.youdao, enabled },
                },
              }));
              void saveConfig();
            }}
            status="connected"
            badges={[{ label: '免费', variant: 'success' }]}
            description="有道提供的翻译服务"
          >
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
          </EngineCard>

          {/* 彩云小译 */}
          <EngineCard
            name="彩云小译"
            icon="/icons/caiyun.svg"
            enabled={config.engines.caiyun?.enabled || false}
            onToggle={(enabled) => {
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
            }}
            status={config.engines.caiyun?.apiToken ? 'connected' : 'warning'}
            badges={[
              { label: '免费额度', variant: 'success' },
              { label: '需要配置', variant: 'warning' },
            ]}
            description="擅长长文本和小说翻译，免费额度100万字/月"
          >
            <div className="mt-3 space-y-3">
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">
                  API Token
                </label>
                <div className="flex gap-2">
                  <div className="relative flex-1">
                    <input
                      type={showSecrets.caiyun ? 'text' : 'password'}
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
                      onClick={() => toggleSecret('caiyun')}
                      className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
                    >
                      {showSecrets.caiyun ? <EyeOff size={16} /> : <Eye size={16} />}
                    </button>
                  </div>
                </div>
              </div>
              <a
                href="https://dashboard.caiyunapp.com/user/sign_in/"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-sm text-primary hover:underline"
              >
                获取 API Token
                <ExternalLink size={12} />
              </a>
            </div>
          </EngineCard>

          {/* DeepL */}
          <EngineCard
            name="DeepL"
            icon="/icons/deepl.svg"
            enabled={config.engines.deepl.enabled || false}
            onToggle={(enabled) => {
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
            }}
            status={config.engines.deepl.apiKey ? 'connected' : 'warning'}
            badges={[
              { label: '付费', variant: 'warning' },
              { label: '需要API Key', variant: 'warning' },
            ]}
            description="高质量机器翻译服务"
          >
            <div className="mt-3 space-y-3">
              <div>
                <label className="block text-sm font-medium text-text-primary mb-2">API Key</label>
                <div className="relative">
                  <input
                    type={showSecrets.deepl ? 'text' : 'password'}
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
                    onClick={() => toggleSecret('deepl')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-text-secondary hover:text-text-primary"
                  >
                    {showSecrets.deepl ? <EyeOff size={16} /> : <Eye size={16} />}
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
                获取 API Key
                <ExternalLink size={12} />
              </a>
            </div>
          </EngineCard>

          {/* 其他引擎可以继续添加... */}
        </div>
      </Card>
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
  icon,
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

  return (
    <div className="p-4 border border-border rounded-lg bg-bg-primary">
      <div className="flex items-start justify-between">
        <div className="flex items-start gap-3 flex-1">
          {/* 引擎图标 */}
          <div className="w-10 h-10 rounded-lg bg-bg-secondary flex items-center justify-center shrink-0">
            <span className="text-2xl">
              {icon === '/icons/google.svg'
                ? '🌐'
                : icon === '/icons/youdao.svg'
                  ? '📘'
                  : icon === '/icons/caiyun.svg'
                    ? '☁️'
                    : icon === '/icons/deepl.svg'
                      ? '🔷'
                      : '🔤'}
            </span>
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
