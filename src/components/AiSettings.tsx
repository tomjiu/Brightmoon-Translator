import { useState, useCallback } from 'react';
import { useConfigStore } from '../stores/configStore';
import { useToastStore } from '../stores/toastStore';
import { useI18n } from '../i18n';
import {
  aiExtractTerms,
  aiLearnStyle,
  type AiTermEntry,
  type TranslationStyle,
} from '../services/ai';
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
} from 'lucide-react';

interface AiSettingsProps {
  onTermsExtracted?: (terms: AiTermEntry[]) => void;
}

export default function AiSettings({ onTermsExtracted }: AiSettingsProps) {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const addToast = useToastStore((s) => s.addToast);

  const [expandedSection, setExpandedSection] = useState<string | null>('polish');
  const [isExtracting, setIsExtracting] = useState(false);
  const [isLearning, setIsLearning] = useState(false);
  const [extractedTerms, setExtractedTerms] = useState<AiTermEntry[]>([]);
  const [learnedStyle, setLearnedStyle] = useState<TranslationStyle | null>(null);

  // Sample texts for term extraction
  const [sampleTexts, setSampleTexts] = useState<Array<[string, string]>>([['', '']]);

  // History for style learning
  const [styleHistory, setStyleHistory] = useState<Array<[string, string]>>([
    ['', ''],
    ['', ''],
    ['', ''],
  ]);

  const toggleSection = (section: string) => {
    setExpandedSection(expandedSection === section ? null : section);
  };

  const handleExtractTerms = useCallback(async () => {
    const validTexts = sampleTexts.filter(([s, t]) => s.trim() && t.trim());
    if (validTexts.length === 0) {
      addToast({
        type: 'warning',
        message: t('aiSettings.needSamplePair') || '请添加至少一对翻译文本',
        duration: 3000,
      });
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
      addToast({
        type: 'success',
        message:
          t('aiSettings.extractedCount', { count: terms.length }) ||
          `提取了 ${terms.length} 个术语`,
        duration: 3000,
      });
    } catch (err) {
      addToast({
        type: 'error',
        message: t('aiSettings.extractFailed') || '术语提取失败',
        detail: String(err),
        duration: 5000,
      });
    } finally {
      setIsExtracting(false);
    }
  }, [sampleTexts, config, addToast, onTermsExtracted]);

  const handleLearnStyle = useCallback(async () => {
    const validHistory = styleHistory.filter(([s, t]) => s.trim() && t.trim());
    if (validHistory.length < 3) {
      addToast({
        type: 'warning',
        message: t('aiSettings.need3Samples') || '请至少添加3对翻译样本',
        duration: 3000,
      });
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
      addToast({
        type: 'success',
        message: t('aiSettings.styleLearned') || '风格学习完成',
        duration: 3000,
      });
    } catch (err) {
      addToast({
        type: 'error',
        message: t('aiSettings.learnFailed') || '风格学习失败',
        detail: String(err),
        duration: 5000,
      });
    } finally {
      setIsLearning(false);
    }
  }, [styleHistory, config, addToast]);

  const addSampleText = () => {
    setSampleTexts([...sampleTexts, ['', '']]);
  };

  const removeSampleText = (index: number) => {
    setSampleTexts(sampleTexts.filter((_, i) => i !== index));
  };

  const updateSampleText = (index: number, field: 0 | 1, value: string) => {
    const updated = sampleTexts.map((item, i) =>
      i === index ? ([...item] as [string, string]) : item,
    );
    updated[index][field] = value;
    setSampleTexts(updated);
  };

  const updateStyleHistory = (index: number, field: 0 | 1, value: string) => {
    const updated = styleHistory.map((item, i) =>
      i === index ? ([...item] as [string, string]) : item,
    );
    updated[index][field] = value;
    setStyleHistory(updated);
  };

  return (
    <div className="space-y-4">
      <h3 className="text-lg font-semibold text-text-primary flex items-center gap-2">
        <Sparkles className="w-5 h-5 text-primary" />
        {t('aiSettings.title') || 'AI 增强功能'}
      </h3>

      {/* Polish Style Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('polish')}
        >
          <div className="flex items-center gap-2">
            <Wand2 className="w-4 h-4 text-primary" />
            <span className="font-medium">{t('aiSettings.polishStyle') || '润色风格'}</span>
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
                {
                  id: 'natural',
                  label: t('aiSettings.styleNatural') || '自然流畅',
                  desc: '日常表达',
                },
                {
                  id: 'formal',
                  label: t('aiSettings.styleFormal') || '正式专业',
                  desc: '商务学术',
                },
                {
                  id: 'casual',
                  label: t('aiSettings.styleCasual') || '轻松口语',
                  desc: '日常对话',
                },
                {
                  id: 'technical',
                  label: t('aiSettings.styleTechnical') || '技术精确',
                  desc: '专业术语',
                },
                {
                  id: 'literary',
                  label: t('aiSettings.styleLiterary') || '文学优雅',
                  desc: '修辞韵律',
                },
              ].map((style) => (
                <div
                  key={style.id}
                  className="p-3 border border-border rounded-lg hover:border-primary/50 transition-colors cursor-pointer"
                >
                  <div className="font-medium text-sm">{style.label}</div>
                  <div className="text-xs text-text-secondary">{style.desc}</div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* Term Extraction Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('terms')}
        >
          <div className="flex items-center gap-2">
            <BookOpen className="w-4 h-4 text-primary" />
            <span className="font-medium">{t('aiSettings.termExtraction') || '术语提取'}</span>
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

            {/* Sample texts input */}
            <div className="space-y-2">
              <label className="text-sm font-medium">
                {t('aiSettings.sampleTexts') || '翻译样本'}：
              </label>
              {sampleTexts.map(([source, target], index) => (
                <div key={index} className="flex gap-2">
                  <input
                    type="text"
                    value={source}
                    onChange={(e) => updateSampleText(index, 0, e.target.value)}
                    placeholder={t('common.sourceText') || '原文'}
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <input
                    type="text"
                    value={target}
                    onChange={(e) => updateSampleText(index, 1, e.target.value)}
                    placeholder={t('common.targetText') || '译文'}
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <button
                    onClick={() => removeSampleText(index)}
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

            {/* Extract button */}
            <button
              onClick={handleExtractTerms}
              disabled={isExtracting}
              className="w-full px-4 py-2 bg-primary text-white rounded-md hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {isExtracting ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Sparkles className="w-4 h-4" />
              )}
              {isExtracting
                ? t('aiSettings.extracting') || '提取中...'
                : t('aiSettings.extractTerms') || '提取术语'}
            </button>

            {/* Extracted terms */}
            {extractedTerms.length > 0 && (
              <div className="space-y-2">
                <label className="text-sm font-medium">
                  {t('aiSettings.extractResult') || '提取结果'}：
                </label>
                <div className="max-h-48 overflow-y-auto space-y-1">
                  {extractedTerms.map((term, index) => (
                    <div
                      key={index}
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

      {/* Style Learning Section */}
      <div className="border border-border rounded-lg overflow-hidden">
        <button
          className="w-full px-4 py-3 flex items-center justify-between bg-bg-secondary hover:bg-bg-tertiary transition-colors"
          onClick={() => toggleSection('style')}
        >
          <div className="flex items-center gap-2">
            <Palette className="w-4 h-4 text-primary" />
            <span className="font-medium">{t('aiSettings.styleLearning') || '风格学习'}</span>
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

            {/* History input */}
            <div className="space-y-2">
              <label className="text-sm font-medium">
                {t('aiSettings.sampleTexts') || '翻译样本'}（至少3对）：
              </label>
              {styleHistory.map(([source, target], index) => (
                <div key={index} className="flex gap-2">
                  <input
                    type="text"
                    value={source}
                    onChange={(e) => updateStyleHistory(index, 0, e.target.value)}
                    placeholder={t('common.sourceText') || '原文'}
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                  <input
                    type="text"
                    value={target}
                    onChange={(e) => updateStyleHistory(index, 1, e.target.value)}
                    placeholder={t('common.targetText') || '译文'}
                    className="flex-1 px-3 py-2 bg-bg-primary border border-border rounded-md text-sm"
                  />
                </div>
              ))}
            </div>

            {/* Learn button */}
            <button
              onClick={handleLearnStyle}
              disabled={isLearning}
              className="w-full px-4 py-2 bg-primary text-white rounded-md hover:bg-primary/90 disabled:opacity-50 transition-colors flex items-center justify-center gap-2"
            >
              {isLearning ? (
                <RefreshCw className="w-4 h-4 animate-spin" />
              ) : (
                <Palette className="w-4 h-4" />
              )}
              {isLearning
                ? t('aiSettings.learning') || '学习中...'
                : t('aiSettings.learnStyle') || '学习风格'}
            </button>

            {/* Learned style */}
            {learnedStyle && (
              <div className="p-4 bg-bg-secondary rounded-lg space-y-3">
                <h4 className="font-medium text-sm">
                  {t('aiSettings.learnResult') || '学习结果'}：
                </h4>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <span className="text-xs text-text-secondary">
                      {t('aiSettings.vocabLevel') || '词汇难度'}
                    </span>
                    <div className="text-sm font-medium">{learnedStyle.vocabularyLevel}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">
                      {t('aiSettings.formality') || '正式程度'}
                    </span>
                    <div className="text-sm font-medium">{learnedStyle.formality}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">
                      {t('aiSettings.sentenceStructure') || '句式特点'}
                    </span>
                    <div className="text-sm font-medium">{learnedStyle.sentenceStructure}</div>
                  </div>
                  <div>
                    <span className="text-xs text-text-secondary">
                      {t('aiSettings.tone') || '语气特征'}
                    </span>
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
