import { useState, useCallback, useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { useToastStore } from '../stores/toastStore';
import {
  fetchAvailableModels,
  testLlmConnection,
  PROVIDER_PRESETS,
  type ModelInfo,
  type LlmProviderEntry,
  type LlmApiFormat,
} from '../services/modelProvider';
import {
  ChevronDown,
  ChevronUp,
  Plus,
  Trash2,
  RefreshCw,
  Eye,
  EyeOff,
  CheckCircle,
  Zap,
  ArrowUpDown,
  Loader2,
} from 'lucide-react';
import AiTranslateTools from '../pages/settings/engines/AiTranslateTools';

interface AiSettingsProps {
  /** Jump to another settings section */
  onNavigate?: (sectionId: string) => void;
}

export default function AiSettings({ onNavigate }: AiSettingsProps) {
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const addToast = useToastStore((s) => s.addToast);

  const [expandedSection, setExpandedSection] = useState<string | null>('llm');
  const [showApiKey, setShowApiKey] = useState<Record<string, boolean>>({});
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [fetchingModels, setFetchingModels] = useState<Record<string, boolean>>({});
  const [testingConnection, setTestingConnection] = useState<Record<string, boolean>>({});
  const [availableModels, setAvailableModels] = useState<Record<string, ModelInfo[]>>({});

  // 提取当前 providers 列表（兼容旧配置 / 残缺 llm）
  const llm = config.llm ?? {
    provider: '',
    apiKey: '',
    apiKeys: [],
    baseUrl: '',
    model: '',
    providers: [],
  };
  const providers: LlmProviderEntry[] =
    Array.isArray(llm.providers) && llm.providers.length
      ? (llm.providers as Array<LlmProviderEntry & { apiFormat?: LlmApiFormat }>).map((p) => ({
          ...p,
          // S3: apiFormat is required in the wire contract; old configs lack it.
          apiFormat: p.apiFormat ?? 'openai',
        }))
      : llm.apiKey || llm.baseUrl
        ? [
            {
              id: 'default',
              name: llm.provider || '自定义',
              baseUrl: llm.baseUrl || '',
              apiKey: llm.apiKey || '',
              model: llm.model || '',
              priority: 0,
              enabled: true,
              models: [],
              apiFormat: 'openai',
            },
          ]
        : [];

  const [editProviders, setEditProviders] = useState<LlmProviderEntry[]>(providers);

  useEffect(() => {
    setEditProviders(providers);
  }, [llm.providers, llm.apiKey, llm.baseUrl, llm.model]);

  const toggleSection = (section: string) => {
    setExpandedSection(expandedSection === section ? null : section);
  };

  // ====== 提供商管理 ======

  const addProvider = (preset?: (typeof PROVIDER_PRESETS)[0]) => {
    const id = preset?.id ? `${preset.id}_${Date.now()}` : `provider_${Date.now()}`;
    const newProvider: LlmProviderEntry = {
      id,
      name: preset?.name || '自定义',
      baseUrl: preset?.baseUrl || '',
      apiKey: '',
      model: preset?.model || '',
      priority: editProviders.length,
      enabled: true,
      models: [],
      apiFormat: preset?.apiFormat || 'openai',
    };
    setEditProviders([...editProviders, newProvider]);
  };

  const removeProvider = (id: string) => {
    setEditProviders(editProviders.filter((p) => p.id !== id));
  };

  const updateProvider = <K extends keyof LlmProviderEntry>(
    id: string,
    field: K,
    value: LlmProviderEntry[K],
  ) => {
    setEditProviders(editProviders.map((p) => (p.id === id ? { ...p, [field]: value } : p)));
  };

  const moveProvider = (id: string, direction: 'up' | 'down') => {
    const idx = editProviders.findIndex((p) => p.id === id);
    if (idx < 0) return;
    const newIdx = direction === 'up' ? idx - 1 : idx + 1;
    if (newIdx < 0 || newIdx >= editProviders.length) return;
    const arr = [...editProviders];
    [arr[idx], arr[newIdx]] = [arr[newIdx], arr[idx]];
    arr.forEach((p, i) => (p.priority = i));
    setEditProviders(arr);
  };

  const fetchModels = async (provider: LlmProviderEntry) => {
    if (!provider.baseUrl) return;
    setFetchingModels((prev) => ({ ...prev, [provider.id]: true }));
    try {
      const models = await fetchAvailableModels(
        provider.baseUrl,
        provider.apiKey,
        provider.apiFormat || 'openai',
      );
      setAvailableModels((prev) => ({ ...prev, [provider.id]: models }));
      updateProvider(
        provider.id,
        'models',
        models.map((m) => m.id),
      );
      addToast({
        type: 'success',
        message: `获取到 ${models.length} 个模型`,
        duration: 3000,
      });
    } catch (err) {
      addToast({
        type: 'error',
        message: `获取模型列表失败: ${err}`,
        duration: 5000,
      });
    } finally {
      setFetchingModels((prev) => ({ ...prev, [provider.id]: false }));
    }
  };

  const handleTestConnection = async (provider: LlmProviderEntry) => {
    if (!provider.baseUrl || !provider.model) return;
    setTestingConnection((prev) => ({ ...prev, [provider.id]: true }));
    try {
      const result = await testLlmConnection(
        provider.baseUrl,
        provider.apiKey,
        provider.model,
        provider.apiFormat || 'openai',
      );
      addToast({ type: 'success', message: result, duration: 5000 });
    } catch (err) {
      addToast({ type: 'error', message: `连接失败: ${err}`, duration: 5000 });
    } finally {
      setTestingConnection((prev) => ({ ...prev, [provider.id]: false }));
    }
  };

  const handleSaveProviders = useCallback(async () => {
    setSaveStatus('saving');
    try {
      const primary = editProviders.find((p) => p.enabled) || editProviders[0];
      updateConfig((prev) => ({
        ...prev,
        llm: {
          ...prev.llm,
          providers: editProviders,
          apiKey: primary.apiKey ?? prev.llm.apiKey,
          baseUrl: primary.baseUrl ?? prev.llm.baseUrl,
          model: primary.model ?? prev.llm.model,
          provider: primary.name ?? prev.llm.provider,
        },
      }));
      await saveConfig();
      setSaveStatus('saved');
      addToast({ type: 'success', message: 'LLM 配置已保存', duration: 3000 });
      setTimeout(() => setSaveStatus('idle'), 2000);
    } catch {
      setSaveStatus('idle');
    }
  }, [editProviders, updateConfig, saveConfig, addToast]);

  // ====== 渲染 ======

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border bg-bg-secondary p-3 space-y-2">
        <p className="text-xs text-text-secondary leading-relaxed">
          密钥与模型只在本页编辑。要让大模型参与主页对比 / OCR
          回退，请在「翻译引擎」把「大模型翻译」排进优先级并保证已配置。
        </p>
        {onNavigate && (
          <button
            type="button"
            onClick={() => onNavigate('engines')}
            className="text-xs font-medium text-primary hover:underline"
          >
            去「翻译引擎」调整顺序 →
          </button>
        )}
      </div>

      {/* ==================== LLM 多提供商配置 ==================== */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('llm')}
        >
          <div className="flex items-center gap-2">
            <Zap className="w-4 h-4 text-primary" />
            <span className="font-medium">API 与模型</span>
            <span className="text-xs text-text-secondary">({editProviders.length} 个提供商)</span>
            {editProviders.some((p) => p.enabled && p.apiKey) && (
              <CheckCircle className="w-3.5 h-3.5 text-green-500" />
            )}
          </div>
          {expandedSection === 'llm' ? (
            <ChevronUp className="w-4 h-4 text-text-secondary" />
          ) : (
            <ChevronDown className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        {expandedSection === 'llm' && (
          <div className="p-4 space-y-4">
            <p className="text-sm text-text-secondary">
              选内置模板填 Key，或自定义并选择请求格式（OpenAI / Anthropic /
              Gemini）。按列表顺序回退。
            </p>

            {/* 提供商列表 */}
            {editProviders.map((provider, idx) => (
              <div
                key={provider.id}
                className="border border-border rounded-lg p-4 space-y-3 bg-bg-primary"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-mono text-text-tertiary w-5">#{idx + 1}</span>
                    <input
                      type="text"
                      value={provider.name}
                      onChange={(e) => updateProvider(provider.id, 'name', e.target.value)}
                      className="px-2 py-1 bg-bg-secondary border border-border rounded text-sm font-medium w-28"
                    />
                    <label className="flex items-center gap-1 text-xs">
                      <input
                        type="checkbox"
                        checked={provider.enabled}
                        onChange={(e) => updateProvider(provider.id, 'enabled', e.target.checked)}
                        className="rounded"
                      />
                      启用
                    </label>
                  </div>
                  <div className="flex items-center gap-1">
                    <button
                      onClick={() => moveProvider(provider.id, 'up')}
                      disabled={idx === 0}
                      className="p-1 text-text-secondary hover:text-text-primary disabled:opacity-30"
                      title="上移"
                    >
                      <ArrowUpDown className="w-3.5 h-3.5" />
                    </button>
                    <button
                      onClick={() => removeProvider(provider.id)}
                      className="p-1 text-red-400 hover:text-red-300"
                      title="删除"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                {/* 请求格式 */}
                <div>
                  <label className="text-xs text-text-secondary mb-1 block">请求格式</label>
                  <select
                    value={provider.apiFormat || 'openai'}
                    onChange={(e) =>
                      updateProvider(provider.id, 'apiFormat', e.target.value as LlmApiFormat)
                    }
                    className="w-full px-2.5 py-1.5 bg-bg-secondary border border-border rounded text-sm"
                  >
                    <option value="openai">OpenAI 兼容（/chat/completions）</option>
                    <option value="anthropic">Anthropic（/messages）</option>
                    <option value="gemini">Google Gemini（generateContent）</option>
                  </select>
                </div>

                {/* API 地址 + 拉取模型 */}
                <div className="flex gap-2">
                  <div className="flex-1">
                    <label className="text-xs text-text-secondary mb-1 block">API 地址</label>
                    <input
                      type="text"
                      value={provider.baseUrl}
                      onChange={(e) => updateProvider(provider.id, 'baseUrl', e.target.value)}
                      placeholder={
                        (provider.apiFormat || 'openai') === 'gemini'
                          ? 'https://generativelanguage.googleapis.com/v1beta'
                          : (provider.apiFormat || 'openai') === 'anthropic'
                            ? 'https://api.anthropic.com/v1'
                            : 'https://api.deepseek.com/v1'
                      }
                      className="w-full px-2.5 py-1.5 bg-bg-secondary border border-border rounded text-sm"
                    />
                  </div>
                  <button
                    onClick={() => fetchModels(provider)}
                    disabled={fetchingModels[provider.id] || !provider.baseUrl}
                    className="self-end px-3 py-1.5 bg-bg-secondary border border-border rounded text-xs hover:bg-bg-tertiary disabled:opacity-50 flex items-center gap-1"
                  >
                    {fetchingModels[provider.id] ? (
                      <Loader2 className="w-3 h-3 animate-spin" />
                    ) : (
                      <RefreshCw className="w-3 h-3" />
                    )}
                    拉取模型
                  </button>
                </div>

                {/* API Key */}
                <div>
                  <label className="text-xs text-text-secondary mb-1 block">API Key</label>
                  <div className="relative">
                    <input
                      type={showApiKey[provider.id] ? 'text' : 'password'}
                      value={provider.apiKey}
                      onChange={(e) => updateProvider(provider.id, 'apiKey', e.target.value)}
                      placeholder="sk-..."
                      className="w-full px-2.5 py-1.5 pr-8 bg-bg-secondary border border-border rounded text-sm"
                    />
                    <button
                      onClick={() =>
                        setShowApiKey((prev) => ({ ...prev, [provider.id]: !prev[provider.id] }))
                      }
                      className="absolute right-2 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
                    >
                      {showApiKey[provider.id] ? (
                        <EyeOff className="w-3.5 h-3.5" />
                      ) : (
                        <Eye className="w-3.5 h-3.5" />
                      )}
                    </button>
                  </div>
                </div>

                {/* 模型选择 */}
                <div>
                  <label className="text-xs text-text-secondary mb-1 block">模型</label>
                  {((availableModels[provider.id] ?? []).length ?? 0) > 0 ? (
                    <select
                      value={provider.model}
                      onChange={(e) => updateProvider(provider.id, 'model', e.target.value)}
                      className="w-full px-2.5 py-1.5 bg-bg-secondary border border-border rounded text-sm"
                    >
                      <option value="">选择模型...</option>
                      {(availableModels[provider.id] ?? []).map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.id} {m.ownedBy ? `(${m.ownedBy})` : ''}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <input
                      type="text"
                      value={provider.model}
                      onChange={(e) => updateProvider(provider.id, 'model', e.target.value)}
                      placeholder="deepseek-chat"
                      className="w-full px-2.5 py-1.5 bg-bg-secondary border border-border rounded text-sm"
                    />
                  )}
                </div>

                {/* 测试连接 */}
                <button
                  onClick={() => handleTestConnection(provider)}
                  disabled={testingConnection[provider.id] || !provider.baseUrl || !provider.model}
                  className="w-full px-3 py-1.5 bg-bg-secondary border border-border rounded text-xs hover:bg-bg-tertiary disabled:opacity-50 flex items-center justify-center gap-1"
                >
                  {testingConnection[provider.id] ? (
                    <Loader2 className="w-3 h-3 animate-spin" />
                  ) : (
                    <Zap className="w-3 h-3" />
                  )}
                  {testingConnection[provider.id] ? '测试中...' : '测试连接'}
                </button>
              </div>
            ))}

            {/* 添加提供商 */}
            <div className="flex gap-2 flex-wrap">
              {PROVIDER_PRESETS.map((preset) => (
                <button
                  key={preset.id}
                  onClick={() => addProvider(preset)}
                  className="flex items-center gap-1 px-3 py-1.5 border border-dashed border-border rounded text-xs hover:border-primary hover:text-primary transition-colors"
                  title={`${preset.apiFormat} · ${preset.baseUrl}`}
                >
                  <Plus className="w-3 h-3" />
                  {preset.name}
                </button>
              ))}
              <button
                onClick={() => addProvider()}
                className="flex items-center gap-1 px-3 py-1.5 border border-dashed border-border rounded text-xs hover:border-primary hover:text-primary transition-colors"
              >
                <Plus className="w-3 h-3" />
                自定义
              </button>
            </div>

            <p className="text-xs text-text-secondary">
              已启用的 Provider 按 priority 参与路由故障转移；请保证至少一项已启用且填写 Key。
            </p>

            {/* AI 学习系统专用模型 */}
            <div className="p-3 bg-bg-secondary border border-border rounded-lg">
              <label className="block text-xs text-text-secondary mb-1">
                AI 学习专用模型（词汇卡 / 出题 / 批量预生成）
              </label>
              <select
                value={config.learnLlmProviderId || ''}
                onChange={(e) => {
                  updateConfig((prev) => ({
                    ...prev,
                    learnLlmProviderId: e.target.value,
                  }));
                  void saveConfig();
                }}
                className="w-full px-2.5 py-1.5 bg-bg-tertiary border border-border rounded text-sm"
              >
                <option value="">跟随全局（与翻译共用）</option>
                {editProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name || p.id}
                    {p.model ? ` · ${p.model}` : ''}
                    {!p.enabled ? '（未启用）' : ''}
                  </option>
                ))}
              </select>
              <p className="text-xs text-text-secondary mt-1">
                AI 学习系统与翻译是两个系统，仅共用服务商列表；此处可为学习系统单独指定模型，留空则与翻译一致。
              </p>
            </div>

            {/* 保存按钮 */}
            <button
              onClick={handleSaveProviders}
              disabled={saveStatus === 'saving'}
              className="w-full px-4 py-2.5 bg-primary text-primary-fg rounded-lg hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {saveStatus === 'saving' ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : saveStatus === 'saved' ? (
                <CheckCircle className="w-4 h-4" />
              ) : null}
              {saveStatus === 'saved'
                ? '已保存 ✓'
                : saveStatus === 'saving'
                  ? '保存中...'
                  : '保存所有配置'}
            </button>

            {/* 回退机制说明 */}
            <div className="p-3 bg-bg-secondary border border-border rounded-lg">
              <h4 className="text-xs font-medium text-text-primary mb-1">⚡ 回退机制</h4>
              <ul className="text-xs text-text-secondary space-y-0.5">
                <li>• 按优先级从上到下尝试</li>
                <li>• 主提供商失败时自动切换到下一个</li>
                <li>• 所有提供商都失败时返回错误</li>
                <li>• 可拖动调整优先级顺序</li>
              </ul>
            </div>
          </div>
        )}
      </div>

      {/* ==================== AI 翻译工具（提示词/润色/术语/风格） ==================== */}
      <div className="rounded-xl border border-border bg-bg-secondary p-3 space-y-2">
        <p className="text-xs text-text-secondary leading-relaxed">
          以下工具为 LLM 翻译（提示词 / 润色 / 术语 / 风格）专用，作用于「翻译引擎」里的大模型翻译，不影响 AI
          学习专用模型（词汇卡 / 出题 / 批量预生成）。
        </p>
      </div>
      <div className="space-y-3">
        <AiTranslateTools config={config} updateConfig={updateConfig} saveConfig={saveConfig} />
      </div>
    </div>
  );
}
