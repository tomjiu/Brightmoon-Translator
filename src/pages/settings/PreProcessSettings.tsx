// PreProcessSettings - 正则预处理管道设置（Phase 2.4）
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Plus, Trash2, Play, ToggleLeft, ToggleRight, Regex, Type } from 'lucide-react';
import Card from '../../components/Card';

interface PreProcessRule {
  id: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  isRegex: boolean;
  langPair?: string;
}

interface PreProcessConfig {
  rules: PreProcessRule[];
  trimWhitespace: boolean;
  normalizeUnicode: boolean;
  removeControlChars: boolean;
}

export default function PreProcessSettings() {
  const [config, setConfig] = useState<PreProcessConfig | null>(null);
  const [testInput, setTestInput] = useState('');
  const [testOutput, setTestOutput] = useState('');
  const [testLangPair, setTestLangPair] = useState('');
  const [showAddForm, setShowAddForm] = useState(false);
  const [newPattern, setNewPattern] = useState('');
  const [newReplacement, setNewReplacement] = useState('');
  const [newIsRegex, setNewIsRegex] = useState(false);
  const [newLangPair, setNewLangPair] = useState('');

  const loadConfig = useCallback(async () => {
    try {
      const data = await invoke<PreProcessConfig>('get_pre_process_config');
      setConfig(data);
    } catch (err) {
      console.error('Failed to load pre-process config:', err);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const saveConfig = async (updated: PreProcessConfig) => {
    try {
      await invoke('update_pre_process_config', { config: updated });
      setConfig(updated);
    } catch (err) {
      console.error('Failed to save pre-process config:', err);
    }
  };

  const handleToggleGlobal = async (key: keyof PreProcessConfig) => {
    if (!config) return;
    const updated = { ...config, [key]: !config[key] };
    await saveConfig(updated);
  };

  const handleAddRule = async () => {
    if (!newPattern) return;
    try {
      await invoke('add_pre_process_rule', {
        pattern: newPattern,
        replacement: newReplacement,
        isRegex: newIsRegex,
        langPair: newLangPair || null,
      });
      setNewPattern('');
      setNewReplacement('');
      setNewIsRegex(false);
      setNewLangPair('');
      setShowAddForm(false);
      await loadConfig();
    } catch (err) {
      console.error('Failed to add rule:', err);
    }
  };

  const handleDeleteRule = async (id: string) => {
    try {
      await invoke('remove_pre_process_rule', { id });
      await loadConfig();
    } catch (err) {
      console.error('Failed to delete rule:', err);
    }
  };

  const handleToggleRule = async (rule: PreProcessRule) => {
    try {
      await invoke('update_pre_process_rule', {
        id: rule.id,
        pattern: rule.pattern,
        replacement: rule.replacement,
        enabled: !rule.enabled,
        isRegex: rule.isRegex,
        langPair: rule.langPair || null,
      });
      await loadConfig();
    } catch (err) {
      console.error('Failed to toggle rule:', err);
    }
  };

  const handleTest = async () => {
    if (!testInput) return;
    try {
      const result = await invoke<string>('test_pre_process', {
        text: testInput,
        langPair: testLangPair || null,
      });
      setTestOutput(result);
    } catch (err) {
      console.error('Failed to test pre-process:', err);
    }
  };

  if (!config) {
    return <div className="text-text-secondary p-6">加载中...</div>;
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">预处理规则</h1>
        <p className="ui-page-desc">翻译前对原文进行正则替换和文本清洗</p>
      </div>

      {/* Global Options */}
      <Card title="全局选项" description="翻译前的文本清洗选项">
        <div className="space-y-3">
          <ToggleRow
            label="去除首尾空白"
            description="翻译前自动 trim"
            checked={config.trimWhitespace}
            onChange={() => void handleToggleGlobal('trimWhitespace')}
          />
          <ToggleRow
            label="Unicode 归一化"
            description="全角字符 → 半角（Ａ→A，！→!）"
            checked={config.normalizeUnicode}
            onChange={() => void handleToggleGlobal('normalizeUnicode')}
          />
          <ToggleRow
            label="移除控制字符"
            description="过滤 ASCII 控制字符（保留换行/制表）"
            checked={config.removeControlChars}
            onChange={() => void handleToggleGlobal('removeControlChars')}
          />
        </div>
      </Card>

      {/* Rules */}
      <Card title={`替换规则 (${config.rules.length})`} description="按顺序执行的文本替换规则">
        <div className="space-y-3">
          {config.rules.map((rule) => (
            <div
              key={rule.id}
              className={`flex items-center gap-3 p-3 rounded-lg border ${
                rule.enabled
                  ? 'bg-bg-primary border-border'
                  : 'bg-bg-tertiary border-border opacity-60'
              }`}
            >
              <button
                onClick={() => void handleToggleRule(rule)}
                className="text-text-secondary hover:text-primary"
                title={rule.enabled ? '禁用' : '启用'}
              >
                {rule.enabled ? (
                  <ToggleRight size={20} className="text-primary" />
                ) : (
                  <ToggleLeft size={20} />
                )}
              </button>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  {rule.isRegex ? (
                    <Regex size={14} className="text-primary shrink-0" />
                  ) : (
                    <Type size={14} className="text-green-500 shrink-0" />
                  )}
                  <code className="text-xs text-text-primary bg-bg-tertiary px-1.5 py-0.5 rounded truncate">
                    {rule.pattern}
                  </code>
                  <span className="text-text-tertiary text-xs">→</span>
                  <code className="text-xs text-text-primary bg-bg-tertiary px-1.5 py-0.5 rounded truncate">
                    {rule.replacement || '(空)'}
                  </code>
                </div>
                {rule.langPair && rule.langPair !== 'all' && (
                  <span className="text-xs text-text-tertiary mt-1 inline-block">
                    仅 {rule.langPair}
                  </span>
                )}
              </div>

              <button
                onClick={() => void handleDeleteRule(rule.id)}
                className="p-1.5 text-text-tertiary hover:text-red-500 rounded"
                title="删除"
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}

          {config.rules.length === 0 && (
            <p className="text-sm text-text-tertiary text-center py-4">暂无替换规则</p>
          )}

          {/* Add Rule */}
          {showAddForm ? (
            <div className="p-3 border border-primary/30 rounded-lg bg-bg-primary space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-text-secondary mb-1">
                    匹配模式
                  </label>
                  <input
                    type="text"
                    value={newPattern}
                    onChange={(e) => setNewPattern(e.target.value)}
                    placeholder={newIsRegex ? '\\d+' : '原始文本'}
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none font-mono"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-text-secondary mb-1">
                    替换为
                  </label>
                  <input
                    type="text"
                    value={newReplacement}
                    onChange={(e) => setNewReplacement(e.target.value)}
                    placeholder="替换文本"
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none font-mono"
                  />
                </div>
              </div>
              <div className="flex items-center gap-4">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={newIsRegex}
                    onChange={(e) => setNewIsRegex(e.target.checked)}
                    className="rounded"
                  />
                  <span className="text-text-secondary">正则表达式</span>
                </label>
                <div className="flex-1">
                  <input
                    type="text"
                    value={newLangPair}
                    onChange={(e) => setNewLangPair(e.target.value)}
                    placeholder="语言对过滤 (如 ja-zh)，留空对所有语言生效"
                    className="w-full px-3 py-1.5 text-xs bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none"
                  />
                </div>
              </div>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowAddForm(false)}
                  className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary"
                >
                  取消
                </button>
                <button
                  onClick={() => void handleAddRule()}
                  disabled={!newPattern}
                  className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50"
                >
                  添加
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setShowAddForm(true)}
              className="w-full flex items-center justify-center gap-2 py-2.5 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary transition-colors"
            >
              <Plus size={16} />
              <span className="text-sm">添加替换规则</span>
            </button>
          )}
        </div>
      </Card>

      {/* Test */}
      <Card title="测试" description="测试预处理效果">
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-text-secondary mb-1">输入文本</label>
              <textarea
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                placeholder="输入要测试的文本..."
                rows={3}
                className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none resize-none"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-text-secondary mb-1">输出结果</label>
              <textarea
                value={testOutput}
                readOnly
                placeholder="点击测试查看结果..."
                rows={3}
                className="w-full px-3 py-2 text-sm bg-bg-primary text-text-primary border border-border rounded resize-none"
              />
            </div>
          </div>
          <div className="flex items-center gap-3">
            <input
              type="text"
              value={testLangPair}
              onChange={(e) => setTestLangPair(e.target.value)}
              placeholder="语言对 (如 ja-zh)"
              className="px-3 py-1.5 text-xs bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none w-40"
            />
            <button
              onClick={() => void handleTest()}
              disabled={!testInput}
              className="flex items-center gap-1.5 px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50"
            >
              <Play size={14} />
              测试
            </button>
          </div>
        </div>
      </Card>
    </div>
  );
}

function ToggleRow({
  label,
  description,
  checked,
  onChange,
}: {
  label: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <label className="flex items-center justify-between cursor-pointer">
      <div>
        <p className="text-sm font-medium text-text-primary">{label}</p>
        <p className="text-xs text-text-secondary">{description}</p>
      </div>
      <div
        onClick={onChange}
        className={`w-10 h-5 rounded-full transition-colors relative ${checked ? 'bg-primary' : 'bg-bg-tertiary'}`}
      >
        <div
          className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform ${checked ? 'translate-x-5' : 'translate-x-0.5'}`}
        />
      </div>
    </label>
  );
}
