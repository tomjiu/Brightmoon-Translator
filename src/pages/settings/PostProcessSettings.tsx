// PostProcessSettings - translation post-process + quality checks
import { useState, useEffect, useCallback } from 'react';
import { invokeOrThrow } from '../../services/invoke';
import { Plus, Trash2, Play, ToggleLeft, ToggleRight, Regex, Type } from 'lucide-react';
import Card from '../../components/Card';
import { useI18n } from '../../i18n';

interface ReplacementRule {
  id: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  isRegex: boolean;
}

interface PostProcessConfig {
  rules: ReplacementRule[];
  trimWhitespace: boolean;
  fixPunctuation: boolean;
  fixNewlines: boolean;
  autoCorrect: boolean;
  symbolRepair: boolean;
  responseCheck: boolean;
}

export default function PostProcessSettings() {
  const { t } = useI18n();
  const [config, setConfig] = useState<PostProcessConfig | null>(null);
  const [testInput, setTestInput] = useState('');
  const [testOutput, setTestOutput] = useState('');
  const [showAddForm, setShowAddForm] = useState(false);
  const [newPattern, setNewPattern] = useState('');
  const [newReplacement, setNewReplacement] = useState('');
  const [newIsRegex, setNewIsRegex] = useState(false);

  const loadConfig = useCallback(async () => {
    try {
      const data = await invokeOrThrow<PostProcessConfig>('get_post_process_config');
      setConfig({
        ...data,
        symbolRepair: data.symbolRepair ?? true,
        responseCheck: data.responseCheck ?? true,
        autoCorrect: data.autoCorrect ?? true,
      });
    } catch (err) {
      console.error('Failed to load post-process config:', err);
    }
  }, []);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  const saveConfig = async (updated: PostProcessConfig) => {
    try {
      await invokeOrThrow('update_post_process_config', { config: updated });
      setConfig(updated);
    } catch (err) {
      console.error('Failed to save post-process config:', err);
    }
  };

  const handleToggle = async (key: keyof PostProcessConfig) => {
    if (!config) return;
    if (key === 'rules') return;
    const updated = { ...config, [key]: !config[key] };
    await saveConfig(updated);
  };

  const handleAddRule = async () => {
    if (!newPattern) return;
    try {
      await invokeOrThrow('add_replacement_rule', {
        pattern: newPattern,
        replacement: newReplacement,
        isRegex: newIsRegex,
      });
      setNewPattern('');
      setNewReplacement('');
      setNewIsRegex(false);
      setShowAddForm(false);
      await loadConfig();
    } catch (err) {
      console.error('Failed to add rule:', err);
    }
  };

  const handleDeleteRule = async (id: string) => {
    try {
      await invokeOrThrow('remove_replacement_rule', { id });
      await loadConfig();
    } catch (err) {
      console.error('Failed to delete rule:', err);
    }
  };

  const handleToggleRule = async (rule: ReplacementRule) => {
    try {
      await invokeOrThrow('update_replacement_rule', {
        id: rule.id,
        pattern: rule.pattern,
        replacement: rule.replacement,
        enabled: !rule.enabled,
        isRegex: rule.isRegex,
      });
      await loadConfig();
    } catch (err) {
      console.error('Failed to toggle rule:', err);
    }
  };

  const handleTest = async () => {
    if (!testInput) return;
    try {
      const result = await invokeOrThrow<string>('test_post_process', { text: testInput });
      setTestOutput(result);
    } catch (err) {
      console.error('Failed to test post-process:', err);
    }
  };

  if (!config) {
    return <div className="text-text-secondary p-6">{t('common.loading')}</div>;
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.postProcess.title')}</h1>
        <p className="ui-page-desc">{t('settings.postProcess.hint')}</p>
      </div>

      <Card
        title={t('settings.postProcess.qualityTitle')}
        description={t('settings.postProcess.qualityHint')}
      >
        <div className="space-y-3">
          <ToggleRow
            label={t('settings.postProcess.symbolRepair')}
            description={t('settings.postProcess.symbolRepairHint')}
            checked={config.symbolRepair}
            onChange={() => void handleToggle('symbolRepair')}
          />
          <ToggleRow
            label={t('settings.postProcess.responseCheck')}
            description={t('settings.postProcess.responseCheckHint')}
            checked={config.responseCheck}
            onChange={() => void handleToggle('responseCheck')}
          />
          <ToggleRow
            label={t('settings.postProcess.autoCorrect')}
            description={t('settings.postProcess.autoCorrectHint')}
            checked={config.autoCorrect}
            onChange={() => void handleToggle('autoCorrect')}
          />
        </div>
      </Card>

      <Card title={t('settings.postProcess.title')} description={t('settings.postProcess.hint')}>
        <div className="space-y-3">
          <ToggleRow
            label={t('settings.postProcess.trimWhitespace')}
            description={t('settings.postProcess.trimWhitespaceHint')}
            checked={config.trimWhitespace}
            onChange={() => void handleToggle('trimWhitespace')}
          />
          <ToggleRow
            label={t('settings.postProcess.fixPunctuation')}
            description={t('settings.postProcess.fixPunctuationHint')}
            checked={config.fixPunctuation}
            onChange={() => void handleToggle('fixPunctuation')}
          />
          <ToggleRow
            label={t('settings.postProcess.fixNewlines')}
            description={t('settings.postProcess.fixNewlinesHint')}
            checked={config.fixNewlines}
            onChange={() => void handleToggle('fixNewlines')}
          />
        </div>
      </Card>

      <Card
        title={`${t('settings.postProcess.replacementRules')} (${(config.rules ?? []).length})`}
        description={t('settings.postProcess.hint')}
      >
        <div className="space-y-3">
          {(config.rules ?? []).map((rule) => (
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
                title={
                  rule.enabled ? t('settings.preProcess.disable') : t('settings.preProcess.enable')
                }
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
                    {rule.replacement || t('settings.preProcess.emptyReplace')}
                  </code>
                </div>
              </div>

              <button
                onClick={() => void handleDeleteRule(rule.id)}
                className="p-1.5 text-text-tertiary hover:text-red-500 rounded"
                title={t('common.delete')}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}

          {(config.rules ?? []).length === 0 && (
            <p className="text-sm text-text-tertiary text-center py-4">
              {t('settings.postProcess.noRules')}
            </p>
          )}

          {showAddForm ? (
            <div className="p-3 border border-primary/30 rounded-lg bg-bg-primary space-y-3">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs font-medium text-text-secondary mb-1">
                    {t('settings.postProcess.pattern')}
                  </label>
                  <input
                    type="text"
                    value={newPattern}
                    onChange={(e) => setNewPattern(e.target.value)}
                    placeholder={newIsRegex ? '\\d+' : ''}
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none font-mono"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-text-secondary mb-1">
                    {t('settings.postProcess.replacement')}
                  </label>
                  <input
                    type="text"
                    value={newReplacement}
                    onChange={(e) => setNewReplacement(e.target.value)}
                    className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none font-mono"
                  />
                </div>
              </div>
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={newIsRegex}
                  onChange={(e) => setNewIsRegex(e.target.checked)}
                  className="rounded"
                />
                <span className="text-text-secondary">{t('settings.postProcess.isRegex')}</span>
              </label>
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setShowAddForm(false)}
                  className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary"
                >
                  {t('common.cancel')}
                </button>
                <button
                  onClick={() => void handleAddRule()}
                  disabled={!newPattern}
                  className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50"
                >
                  {t('settings.postProcess.addRule')}
                </button>
              </div>
            </div>
          ) : (
            <button
              onClick={() => setShowAddForm(true)}
              className="w-full flex items-center justify-center gap-2 py-2.5 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary transition-colors"
            >
              <Plus size={16} />
              <span className="text-sm">{t('settings.postProcess.addRule')}</span>
            </button>
          )}
        </div>
      </Card>

      <Card title={t('settings.postProcess.test')} description={t('settings.postProcess.hint')}>
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-text-secondary mb-1">
                {t('settings.postProcess.testInput')}
              </label>
              <textarea
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                rows={3}
                className="w-full px-3 py-2 text-sm bg-bg-tertiary text-text-primary border border-border rounded focus:border-primary outline-none resize-none"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-text-secondary mb-1">
                {t('settings.postProcess.testOutput')}
              </label>
              <textarea
                value={testOutput}
                readOnly
                rows={3}
                className="w-full px-3 py-2 text-sm bg-bg-primary text-text-primary border border-border rounded resize-none"
              />
            </div>
          </div>
          <button
            onClick={() => void handleTest()}
            disabled={!testInput}
            className="flex items-center gap-1.5 px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary/90 disabled:opacity-50"
          >
            <Play size={14} />
            {t('settings.postProcess.test')}
          </button>
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
