import { useState, useCallback, useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { useToastStore } from '../stores/toastStore';
import {
  aiExtractTerms,
  aiLearnStyle,
  type AiTermEntry,
  type TranslationStyle,
} from '../services/ai';
import {
  fetchAvailableModels,
  testLlmConnection,
  PROVIDER_PRESETS,
  type ModelInfo,
  type LlmProviderEntry,
  type LlmApiFormat,
} from '../services/modelProvider';
import {
  Sparkles,
  BookOpen,
  Palette,
  Wand2,
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

interface AiSettingsProps {
  onTermsExtracted?: (terms: AiTermEntry[]) => void;
  /** Jump to another settings section */
  onNavigate?: (sectionId: string) => void;
}

export default function AiSettings({ onTermsExtracted, onNavigate }: AiSettingsProps) {
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

  // 术语提取相关
  const [isExtracting, setIsExtracting] = useState(false);
  const [isLearning, setIsLearning] = useState(false);
  const [extractedTerms, setExtractedTerms] = useState<AiTermEntry[]>([]);
  const [learnedStyle, setLearnedStyle] = useState<TranslationStyle | null>(null);
  const [sampleTexts, setSampleTexts] = useState<Array<[string, string]>>([['', '']]);
  const [styleHistory, setStyleHistory] = useState<Array<[string, string]>>([
    ['', ''],
    ['', ''],
    ['', ''],
  ]);

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

  // ====== 术语提取（原有功能） ======

  const handleExtractTerms = useCallback(async () => {
    const validTexts = sampleTexts.filter(([s, t]) => s.trim() && t.trim());
    if (validTexts.length === 0) {
      addToast({ type: 'warning', message: '请添加至少一对翻译文本', duration: 3000 });
      return;
    }
    setIsExtracting(true);
    try {
      const terms = await aiExtractTerms({
        texts: validTexts,
        fromLang: config.defaultFrom || 'auto',
        toLang: config.defaultTo || 'zh',
      });
      setExtractedTerms(terms);
      onTermsExtracted?.(terms);
      addToast({ type: 'success', message: `提取了 ${terms.length} 个术语`, duration: 3000 });
    } catch (err) {
      addToast({ type: 'error', message: `术语提取失败: ${err}`, duration: 5000 });
    } finally {
      setIsExtracting(false);
    }
  }, [sampleTexts, config, addToast, onTermsExtracted]);

  const handleLearnStyle = useCallback(async () => {
    const validHistory = styleHistory.filter(([s, t]) => s.trim() && t.trim());
    if (validHistory.length < 3) {
      addToast({ type: 'warning', message: '请至少添加3对翻译样本', duration: 3000 });
      return;
    }
    setIsLearning(true);
    try {
      const style = await aiLearnStyle({
        history: validHistory,
        fromLang: config.defaultFrom || 'auto',
        toLang: config.defaultTo || 'zh',
      });
      setLearnedStyle(style);
      addToast({ type: 'success', message: '风格学习完成', duration: 3000 });
    } catch (err) {
      addToast({ type: 'error', message: `风格学习失败: ${err}`, duration: 5000 });
    } finally {
      setIsLearning(false);
    }
  }, [styleHistory, config, addToast]);

  const addSampleText = () => setSampleTexts([...sampleTexts, ['', '']]);
  const removeSampleText = (i: number) => setSampleTexts(sampleTexts.filter((_, idx) => idx !== i));
  const updateSampleText = (i: number, f: 0 | 1, v: string) => {
    const u = sampleTexts.map((x, idx) => (idx === i ? ([...x] as [string, string]) : x));
    u[i][f] = v;
    setSampleTexts(u);
  };
  const updateStyleHistory = (i: number, f: 0 | 1, v: string) => {
    const u = styleHistory.map((x, idx) => (idx === i ? ([...x] as [string, string]) : x));
    u[i][f] = v;
    setStyleHistory(u);
  };

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

      {/* ==================== 润色风格 ==================== */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('polish')}
        >
          <div className="flex items-center gap-2">
            <Wand2 className="w-4 h-4 text-primary" />
            <span className="font-medium">润色风格</span>
          </div>
          {expandedSection === 'polish' ? (
            <ChevronUp className="w-4 h-4 text-text-secondary" />
          ) : (
            <ChevronDown className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        {expandedSection === 'polish' && (
          <div className="p-4 space-y-3">
            <p className="text-sm text-text-secondary">
              AI 润色可以优化翻译结果，使其更加自然流畅。支持多种风格：
            </p>
            <div className="grid grid-cols-2 gap-2">
              {[
                { id: 'natural', label: '自然流畅', desc: '日常表达' },
                { id: 'formal', label: '正式专业', desc: '商务学术' },
                { id: 'casual', label: '轻松口语', desc: '日常对话' },
                { id: 'technical', label: '技术精确', desc: '专业术语' },
                { id: 'literary', label: '文学优雅', desc: '修辞韵律' },
              ].map((s) => (
                <div
                  key={s.id}
                  className="p-3 border border-border rounded-lg hover:border-primary/50 transition-colors cursor-pointer"
                >
                  <div className="font-medium text-sm">{s.label}</div>
                  <div className="text-xs text-text-secondary">{s.desc}</div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* ==================== 术语提取 ==================== */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('terms')}
        >
          <div className="flex items-center gap-2">
            <BookOpen className="w-4 h-4 text-primary" />
            <span className="font-medium">术语提取</span>
          </div>
          {expandedSection === 'terms' ? (
            <ChevronUp className="w-4 h-4 text-text-secondary" />
          ) : (
            <ChevronDown className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        {expandedSection === 'terms' && (
          <div className="p-4 space-y-4">
            <p className="text-sm text-text-secondary">从翻译对中自动提取专业术语，生成术语表。</p>
            <div className="space-y-2">
              <label className="text-sm font-medium">翻译样本：</label>
              {sampleTexts.map((pair, i) => (
                <div key={i} className="flex gap-2">
                  <input
                    type="text"
                    value={pair[0] ?? ''}
                    onChange={(e) => updateSampleText(i, 0, e.target.value)}
                    placeholder="原文"
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <input
                    type="text"
                    value={pair[1] ?? ''}
                    onChange={(e) => updateSampleText(i, 1, e.target.value)}
                    placeholder="译文"
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <button
                    onClick={() => removeSampleText(i)}
                    className="px-2 py-2 text-red-500 hover:bg-red-500/10 rounded-md transition-colors"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
              ))}
              <button
                onClick={addSampleText}
                className="flex items-center gap-1 text-sm text-primary hover:text-primary/80 transition-colors"
              >
                <Plus className="w-4 h-4" />
                添加样本
              </button>
            </div>
            <button
              onClick={handleExtractTerms}
              disabled={isExtracting}
              className="w-full px-4 py-2 bg-primary text-primary-fg rounded-md hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {isExtracting ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Sparkles className="w-4 h-4" />
              )}
              {isExtracting ? '提取中...' : '提取术语'}
            </button>
            {extractedTerms.length > 0 && (
              <div className="space-y-2">
                <label className="text-sm font-medium">提取结果：</label>
                <div className="max-h-48 overflow-y-auto space-y-1">
                  {extractedTerms.map((term, i) => (
                    <div
                      key={i}
                      className="flex items-center justify-between p-2 bg-bg-secondary rounded-md"
                    >
                      <div>
                        <span className="text-sm font-medium">{term.source}</span>
                        <span className="text-sm text-text-secondary mx-2">→</span>
                        <span className="text-sm">{term.target}</span>
                        {term.context && (
                          <span className="text-xs text-text-secondary ml-2">({term.context})</span>
                        )}
                      </div>
                      <span className="text-xs text-text-secondary">
                        {Math.round(term.confidence * 100)}%
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* ==================== 风格学习 ==================== */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('style')}
        >
          <div className="flex items-center gap-2">
            <Palette className="w-4 h-4 text-primary" />
            <span className="font-medium">风格学习</span>
          </div>
          {expandedSection === 'style' ? (
            <ChevronUp className="w-4 h-4 text-text-secondary" />
          ) : (
            <ChevronDown className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        {expandedSection === 'style' && (
          <div className="p-4 space-y-4">
            <p className="text-sm text-text-secondary">
              从您的历史翻译中学习风格特征，应用到新翻译中。
            </p>
            <div className="space-y-2">
              <label className="text-sm font-medium">翻译样本（至少3对）：</label>
              {styleHistory.map(([source, target], i) => (
                <div key={i} className="flex gap-2">
                  <input
                    type="text"
                    value={source}
                    onChange={(e) => updateStyleHistory(i, 0, e.target.value)}
                    placeholder="原文"
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <input
                    type="text"
                    value={target}
                    onChange={(e) => updateStyleHistory(i, 1, e.target.value)}
                    placeholder="译文"
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                </div>
              ))}
            </div>
            <button
              onClick={handleLearnStyle}
              disabled={isLearning}
              className="w-full px-4 py-2 bg-primary text-primary-fg rounded-md hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {isLearning ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Palette className="w-4 h-4" />
              )}
              {isLearning ? '学习中...' : '学习风格'}
            </button>
            {learnedStyle && (
              <div className="p-4 bg-bg-secondary rounded-lg space-y-3">
                <h4 className="font-medium text-sm">学习结果：</h4>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <span className="text-xs text-text-secondary">词汇难度</span>
                    <div className="text-sm font-medium">{learnedStyle.vocabularyLevel}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">正式程度</span>
                    <div className="text-sm font-medium">{learnedStyle.formality}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">句式特点</span>
                    <div className="text-sm font-medium">{learnedStyle.sentenceStructure}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">语气特征</span>
                    <div className="text-sm font-medium">{learnedStyle.tone}</div>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
