import { useState, useCallback } from 'react';
import { useToastStore } from '../../../stores/toastStore';
import {
  aiExtractTerms,
  aiLearnStyle,
  type AiTermEntry,
  type TranslationStyle,
} from '../../../services/ai';
import type { AppConfig } from '../../../types';
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
  CheckCircle,
} from 'lucide-react';

interface AiTranslateToolsProps {
  config: AppConfig;
  updateConfig: (updater: (prev: AppConfig) => AppConfig) => void;
  saveConfig: () => Promise<void>;
}

export default function AiTranslateTools({ config, updateConfig, saveConfig }: AiTranslateToolsProps) {
  const addToast = useToastStore((s) => s.addToast);

  const [expandedSection, setExpandedSection] = useState<string | null>('prompt');
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

  const setPrompt = useCallback(
    (prompt: string) => {
      updateConfig((prev: AppConfig) => ({ ...prev, customPrompt: prompt }));
    },
    [updateConfig],
  );

  const handleSavePrompt = useCallback(() => {
    void saveConfig();
  }, [saveConfig]);

  // ====== 术语提取 ======

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
      addToast({ type: 'success', message: `提取了 ${terms.length} 个术语`, duration: 3000 });
    } catch (err) {
      addToast({ type: 'error', message: `术语提取失败: ${err}`, duration: 5000 });
    } finally {
      setIsExtracting(false);
    }
  }, [sampleTexts, config, addToast]);

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

  const polishStyles = [
    { id: 'natural', label: '自然流畅', desc: '日常表达' },
    { id: 'formal', label: '正式专业', desc: '商务学术' },
    { id: 'casual', label: '轻松口语', desc: '日常对话' },
    { id: 'technical', label: '技术精确', desc: '专业术语' },
    { id: 'literary', label: '文学优雅', desc: '修辞韵律' },
  ];

  return (
    <div className="space-y-3">
      {/* ==================== 翻译提示词 ==================== */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('prompt')}
        >
          <div className="flex items-center gap-2">
            <Wand2 className="w-4 h-4 text-primary" />
            <span className="font-medium">翻译提示词</span>
          </div>
          {expandedSection === 'prompt' ? (
            <ChevronUp className="w-4 h-4 text-text-secondary" />
          ) : (
            <ChevronDown className="w-4 h-4 text-text-secondary" />
          )}
        </button>
        {expandedSection === 'prompt' && (
          <div className="p-4 space-y-3">
            <p className="text-sm text-text-secondary">
              自定义 LLM 翻译的系统提示词。可在其中使用
              <code className="mx-1 px-1 py-0.5 bg-bg-tertiary rounded text-xs">
                {'{from}'}
              </code>
              <code className="mx-1 px-1 py-0.5 bg-bg-tertiary rounded text-xs">
                {'{to}'}
              </code>
              占位符（会被替换为源/目标语言）。留空使用默认提示词。
            </p>
            <textarea
              value={config.customPrompt || ''}
              onChange={(e) => setPrompt(e.target.value)}
              onBlur={handleSavePrompt}
              rows={5}
              placeholder="例：你是一个擅长{from}→{to}翻译的专家，翻译时注意术语一致性，只输出译文。"
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary outline-none font-mono text-xs"
            />
            <div className="flex justify-end">
              <button
                type="button"
                onClick={handleSavePrompt}
                className="flex items-center gap-1 px-3 py-1.5 bg-primary text-primary-fg rounded-lg text-xs hover:bg-primary/90"
              >
                <CheckCircle className="w-3 h-3" />
                保存提示词
              </button>
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
              {polishStyles.map((s) => (
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