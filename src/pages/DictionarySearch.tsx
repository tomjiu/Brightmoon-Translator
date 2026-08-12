// DictionarySearch - 多源聚合词典查询

import { useState, useEffect, useRef, useCallback } from 'react';
import {
  Search,
  Volume2,
  Copy,
  Loader2,
  X,
  Database,
  Globe,
  BookOpen,
  Sparkles,
  Wand2,
  Settings2,
  RefreshCw,
} from 'lucide-react';
import { safeInvoke, invokeOrDefault } from '../services/invoke';
import { speakText } from '../services/tts';
import { detectSpeakLang } from '../utils/speech';
import { useI18n } from '../i18n';
import { saveAndCollect, summarizeReport } from '../hooks/useCollectionPush';
import { extractWordsAndStudy } from '../services/vocabulary';
import { CoreVocabularyList } from '../components/vocabulary';
import {
  getDictSources,
  saveDictSources,
  type DictSourceConfig,
} from '../services/dictionarySource';
import PageHeader from '../components/PageHeader';

interface PhoneticInfo {
  text?: string;
  audio?: string;
  source: string;
}

interface OnlineDefinition {
  definition: string;
  example?: string;
  synonyms: string[];
  antonyms: string[];
}

interface OnlineMeaning {
  partOfSpeech: string;
  definitions: OnlineDefinition[];
}

interface ComprehensiveEntry {
  word: string;
  phonetics: PhoneticInfo[];
  chineseTranslation?: string;
  englishDefinitions: string[];
  oxfordDefinition?: string;
  onlineMeanings: OnlineMeaning[];
  gptAnalysis?: string;
  audioUrl?: string;
  usAudioUrl?: string;
  ukAudioUrl?: string;
  examples: BilingualExample[];
  collinsEntries: CollinsEntry[];
  sources: string[];
}

interface CollinsEntry {
  pos: string;
  posCn: string;
  englishDef: string;
  examples: BilingualExample[];
}

interface BilingualExample {
  en: string;
  zh: string;
}

interface SuggestionItem {
  word: string;
  preview?: string;
}

interface DictionaryHistoryItem {
  word: string;
  lookupCount: number;
  firstLookedUp: number;
  lastLookedUp: number;
}

function DictionarySearch() {
  const [searchQuery, setSearchQuery] = useState('');
  const [suggestions, setSuggestions] = useState<SuggestionItem[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [result, setResult] = useState<ComprehensiveEntry | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSuggestionsLoading, setIsSuggestionsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  // T6 接线:AI 抽生词建本
  const [showExtractDialog, setShowExtractDialog] = useState(false);
  const [extractText, setExtractText] = useState('');
  const [isExtracting, setIsExtracting] = useState(false);
  const [extractResult, setExtractResult] = useState<string | null>(null);
  // T7 接线:词典源管理
  const [showSourcesDialog, setShowSourcesDialog] = useState(false);
  const [dictSources, setDictSources] = useState<DictSourceConfig[]>([]);
  const [isSourcesLoading, setIsSourcesLoading] = useState(false);
  const [sourcesError, setSourcesError] = useState<string | null>(null);
  const [isSavingSources, setIsSavingSources] = useState(false);
  const [sourcesSaved, setSourcesSaved] = useState(false);

  // T7: 加载词典源配置
  const loadDictSources = useCallback(async () => {
    setIsSourcesLoading(true);
    setSourcesError(null);
    try {
      const sources = await getDictSources();
      setDictSources(sources);
    } catch (err) {
      setSourcesError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSourcesLoading(false);
    }
  }, []);

  const openSourcesDialog = () => {
    setShowSourcesDialog(true);
    setSourcesSaved(false);
    void loadDictSources();
  };

  const toggleDictSource = (id: string) => {
    setDictSources((prev) =>
      prev.map((s) => (s.id === id ? { ...s, enabled: !s.enabled } : s)),
    );
  };

  const handleSaveSources = async () => {
    setIsSavingSources(true);
    setSourcesError(null);
    try {
      await saveDictSources(
        dictSources.map((s) => ({
          source_id: s.id,
          enabled: s.enabled,
          priority: s.priority,
          prompt_template: s.prompt_template,
        })),
      );
      setSourcesSaved(true);
    } catch (err) {
      setSourcesError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSavingSources(false);
    }
  };

  // 自动检测并导入词典数据（仅首次）
  useEffect(() => {
    const checkAndImport = async () => {
      const status = await invokeOrDefault<{ imported: boolean; vocabCount: number }>(
        'check_dictionary_imported',
        undefined,
        { imported: false, vocabCount: 0 },
      );
      if (!status.imported) {
        setIsImporting(true);
        setImportStatus('首次使用，正在导入词典数据...');
        const [msg, importErr] = await safeInvoke<string>('import_dictionary_data', undefined, {
          silent: true,
        });
        setImportStatus(importErr ? `导入失败: ${importErr.message}` : msg);
        setIsImporting(false);
      }
    };
    void checkAndImport();
  }, []);
  const [history, setHistory] = useState<DictionaryHistoryItem[]>([]);
  const loadHistory = useCallback(async () => {
    // 数据库未初始化或首次使用 — 静默返回空数组
    const items = await invokeOrDefault<DictionaryHistoryItem[]>(
      'get_dictionary_history',
      { limit: 50 },
      [],
    );
    setHistory(items);
  }, []);

  // 加载持久化查词历史
  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  const handleClearHistory = async () => {
    const [, error] = await safeInvoke('clear_dictionary_history');
    if (error) {
      console.error('清空历史失败:', error.message);
    } else {
      setHistory([]);
    }
  };
  const searchRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  // 实时联想搜索
  useEffect(() => {
    const fetchSuggestions = async () => {
      if (!searchQuery.trim() || searchQuery.length < 2) {
        setSuggestions([]);
        return;
      }
      setIsSuggestionsLoading(true);
      try {
        const data = await invokeOrDefault<SuggestionItem[]>(
          'search_word_suggestions',
          { query: searchQuery.trim(), limit: 10 },
          [],
        );
        setSuggestions(data);
        setShowSuggestions(true);
        setSelectedIndex(-1);
      } finally {
        setIsSuggestionsLoading(false);
      }
    };
    const debounce = setTimeout(fetchSuggestions, 200);
    return () => clearTimeout(debounce);
  }, [searchQuery]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (searchRef.current && !searchRef.current.contains(event.target as Node)) {
        setShowSuggestions(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleLookup = useCallback(async (word: string) => {
    if (!word.trim()) return;
    setIsLoading(true);
    setError(null);
    setShowSuggestions(false);
    try {
      const [data, error] = await safeInvoke<ComprehensiveEntry>('lookup_word_multi_source', {
        word: word.trim(),
        recordHistory: true,
      });
      if (error || !data) {
        setError(error?.message || '查询失败');
        setResult(null);
        return;
      }
      setResult(data);
      setSearchQuery(data.word);
      // 刷新持久化历史（后端 UPSERT 已更新次数与时间）
      void loadHistory();
    } finally {
      setIsLoading(false);
    }
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!showSuggestions || suggestions.length === 0) {
      if (e.key === 'Enter') void handleLookup(searchQuery);
      return;
    }
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex((p) => (p < suggestions.length - 1 ? p + 1 : p));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex((p) => (p > 0 ? p - 1 : -1));
        break;
      case 'Enter':
        e.preventDefault();
        void handleLookup(selectedIndex >= 0 ? suggestions[selectedIndex].word : searchQuery);
        break;
      case 'Escape':
        setShowSuggestions(false);
        break;
    }
  };

  const handleClear = () => {
    setSearchQuery('');
    setResult(null);
    setError(null);
    setSuggestions([]);
    inputRef.current?.focus();
  };

  const playAudio = (url: string) => {
    if (audioRef.current) audioRef.current.pause();
    audioRef.current = new Audio(url);
    audioRef.current.play();
  };

  const playTts = async (text: string) => {
    try {
      await speakText(text, detectSpeakLang(text));
    } catch (err) {
      console.error('TTS 发音失败:', err);
    }
  };

  const handleImport = async () => {
    setIsImporting(true);
    setImportStatus('导入中...');
    const [msg, error] = await safeInvoke<string>('import_dictionary_data');
    setImportStatus(error ? `导入失败: ${error.message}` : msg);
    setIsImporting(false);
  };

  // T6 接线:AI 抽生词建本 handler
  const handleExtractWords = async () => {
    const text = extractText.trim();
    if (!text) return;
    setIsExtracting(true);
    setExtractResult(null);
    try {
      const result = await extractWordsAndStudy(text);
      const studied = result.studied.length;
      const skipped = result.skipped_existing.length;
      const total = result.total_words;
      setExtractResult(
        `完成:共 ${total} 词,新建 ${studied} 张卡,跳过已有 ${skipped} 词`,
      );
      setExtractText('');
    } catch (err) {
      setExtractResult(`失败: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsExtracting(false);
    }
  };

  return (
    <div className="h-full flex flex-col bg-bg-primary">
      {/* Search Bar */}
      <div className="p-6 border-b border-border bg-bg-secondary">
        <div className="max-w-3xl mx-auto">
          <PageHeader
            title="词典查询"
            icon={BookOpen}
            className="mb-4"
            actions={
              <div className="flex items-center gap-2">
                <span className="ui-caption px-2 py-1 bg-bg-tertiary border border-border rounded flex items-center gap-1">
                  <Globe size={12} />
                  多源聚合
                </span>
                <button
                  onClick={() => setShowExtractDialog(true)}
                  className="text-xs px-3 py-1 bg-bg-tertiary text-text-secondary rounded hover:text-primary border border-border flex items-center gap-1"
                  title="粘贴一段文本,AI 自动抽取生词并批量建卡"
                >
                  <Wand2 size={12} />
                  AI 抽生词
                </button>
                <button
                  onClick={openSourcesDialog}
                  className="text-xs px-3 py-1 bg-bg-tertiary text-text-secondary rounded hover:text-primary border border-border flex items-center gap-1"
                  title="管理词典源(ECDICT / 有道 / 在线API / AI Prompt)"
                >
                  <Settings2 size={12} />
                  词典源
                </button>
                <button
                  onClick={handleImport}
                  disabled={isImporting}
                  className="text-xs px-3 py-1 bg-bg-tertiary text-text-secondary rounded hover:text-primary border border-border disabled:opacity-50"
                >
                  {isImporting ? '导入中...' : '重新导入词典'}
                </button>
              </div>
            }
          />
          {importStatus && <p className="text-xs text-primary mb-2">{importStatus}</p>}
          <div ref={searchRef} className="relative">
            <div className="flex gap-2">
              <div className="flex-1 relative">
                <Search
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-text-tertiary"
                  size={20}
                />
                <input
                  ref={inputRef}
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={handleKeyDown}
                  onFocus={() => suggestions.length > 0 && setShowSuggestions(true)}
                  placeholder="输入英文单词查询..."
                  className="w-full pl-10 pr-10 py-3 bg-bg-primary border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary text-text-primary"
                  autoComplete="off"
                />
                {searchQuery && (
                  <button
                    onClick={handleClear}
                    className="absolute right-3 top-1/2 -translate-y-1/2 text-text-tertiary hover:text-text-primary"
                  >
                    <X size={18} />
                  </button>
                )}
              </div>
              <button
                onClick={() => void handleLookup(searchQuery)}
                disabled={isLoading || !searchQuery.trim()}
                className="px-6 py-3 bg-primary text-primary-fg rounded-lg hover:bg-primary/90 disabled:opacity-50 font-medium"
              >
                {isLoading ? <Loader2 className="animate-spin" size={20} /> : '查询'}
              </button>
            </div>
            {showSuggestions && (suggestions.length > 0 || isSuggestionsLoading) && (
              <div className="absolute top-full left-0 right-16 mt-1 bg-bg-secondary border border-border rounded-lg shadow-lg max-h-80 overflow-y-auto z-50">
                {isSuggestionsLoading ? (
                  <div className="p-4 text-center">
                    <Loader2 className="animate-spin inline-block" size={20} />
                  </div>
                ) : (
                  suggestions.map((item, i) => (
                    <button
                      key={item.word}
                      onClick={() => void handleLookup(item.word)}
                      className={`w-full px-4 py-2 text-left hover:bg-bg-tertiary ${i === selectedIndex ? 'bg-bg-tertiary' : ''}`}
                    >
                      <span className="text-text-primary font-medium">{item.word}</span>
                      {item.preview && (
                        <span className="text-xs text-text-tertiary ml-2 truncate max-w-xs inline-block align-bottom">
                          {item.preview}
                        </span>
                      )}
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
          <p className="text-xs text-text-secondary mt-2">
            💡 ECDICT（中文）+ 有道（音频/例句）+ Oxford（权威）+ GPT4（词根）+ DictionaryAPI.dev
          </p>
          {/* 搜索历史 */}
          {history.length > 0 && (
            <div className="flex items-center gap-2 mt-2 flex-wrap">
              <span className="text-xs text-text-tertiary">最近：</span>
              {history.slice(0, 12).map((item) => (
                <button
                  key={item.word}
                  onClick={() => {
                    setSearchQuery(item.word);
                    void handleLookup(item.word);
                  }}
                  className="text-xs px-2 py-0.5 bg-bg-tertiary text-text-secondary rounded hover:text-primary hover:bg-bg-primary border border-border flex items-center gap-1"
                  title={`查询 ${item.lookupCount} 次`}
                >
                  {item.word}
                  {item.lookupCount > 1 && (
                    <span className="text-[10px] text-text-tertiary">×{item.lookupCount}</span>
                  )}
                </button>
              ))}
              <button
                onClick={() => void handleClearHistory()}
                className="text-xs px-2 py-0.5 text-text-tertiary hover:text-red-500 transition-colors"
                title="清空历史"
              >
                清除
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto">
          {error && (
            <div className="bg-red-50 border border-red-200 text-red-700 p-4 rounded-lg mb-4">
              {error}
            </div>
          )}
          {!result && !isLoading && !error && (
            searchQuery ? (
              <div className="text-center text-text-secondary py-12">
                <Search size={48} className="mx-auto mb-4 opacity-50" />
                <p className="text-lg font-medium">按回车查询单词</p>
                <p className="text-sm mt-2">多源聚合：自动合并多个词典的数据</p>
              </div>
            ) : (
              <div className="bg-bg-secondary border border-border rounded-lg overflow-hidden">
                <div className="px-4 py-3 border-b border-border flex items-center justify-between">
                  <h3 className="text-sm font-medium text-text-primary flex items-center gap-2">
                    <Database size={14} />
                    核心词库浏览
                  </h3>
                  <span className="text-xs text-text-tertiary">
                    点击单词直接查询 · 按词频排序
                  </span>
                </div>
                <CoreVocabularyList
                  className="h-64"
                  onSelectWord={(w) => void handleLookup(w)}
                />
              </div>
            )
          )}
          {result && <ResultCard result={result} onPlayAudio={playAudio} onSpeak={playTts} />}
        </div>
      </div>

      {/* T6 接线:AI 抽生词建本对话框 */}
      {showExtractDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
          <div className="ui-card w-full max-w-2xl p-6 animate-fadeIn">
            <div className="flex items-center justify-between mb-4">
              <h3 className="ui-section-title flex items-center gap-2">
                <Wand2 size={16} />
                AI 抽生词建本
              </h3>
              <button
                onClick={() => {
                  setShowExtractDialog(false);
                  setExtractResult(null);
                  setExtractText('');
                }}
                className="text-text-secondary hover:text-text-primary"
              >
                <X size={18} />
              </button>
            </div>
            <p className="ui-caption mb-3">
              粘贴一段英文文本(如文章/句子),系统会自动抽取生词、过滤停用词、查 ECDICT
              验证,然后批量创建学习卡牌。已有卡牌的词会跳过。
            </p>
            <textarea
              value={extractText}
              onChange={(e) => setExtractText(e.target.value)}
              placeholder="Paste English text here... 例如:The quick brown fox jumps over the lazy dog."
              className="w-full h-40 p-3 bg-bg-tertiary border border-border rounded-lg text-sm text-text-primary resize-none focus:outline-none focus:border-primary"
              disabled={isExtracting}
            />
            {extractResult && (
              <p className="text-xs text-primary mt-2 p-2 bg-bg-tertiary rounded">
                {extractResult}
              </p>
            )}
            <div className="flex justify-end gap-2 mt-4">
              <button
                onClick={() => {
                  setShowExtractDialog(false);
                  setExtractResult(null);
                  setExtractText('');
                }}
                className="px-4 py-1.5 text-sm text-text-secondary hover:text-text-primary"
                disabled={isExtracting}
              >
                关闭
              </button>
              <button
                onClick={handleExtractWords}
                disabled={isExtracting || !extractText.trim()}
                className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary-hover disabled:opacity-50 flex items-center gap-1.5"
              >
                {isExtracting ? (
                  <>
                    <Loader2 size={14} className="animate-spin" />
                    抽取中...
                  </>
                ) : (
                  <>
                    <Sparkles size={14} />
                    开始抽取
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* T7 接线:词典源管理对话框 */}
      {showSourcesDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
          <div className="ui-card w-full max-w-lg p-6 animate-fadeIn">
            <div className="flex items-center justify-between mb-4">
              <h3 className="ui-section-title flex items-center gap-2">
                <Settings2 size={16} />
                词典源管理
              </h3>
              <button
                onClick={() => setShowSourcesDialog(false)}
                className="text-text-secondary hover:text-text-primary"
              >
                <X size={18} />
              </button>
            </div>
            <p className="ui-caption mb-4">
              控制词典查询使用哪些来源。AI Prompt 源需配置 LLM 后才会生效。
            </p>

            {isSourcesLoading ? (
              <div className="flex items-center gap-2 text-text-secondary text-sm py-6">
                <Loader2 size={14} className="animate-spin" />
                加载中...
              </div>
            ) : (
              <div className="space-y-2 mb-4">
                {dictSources.map((s) => (
                  <div
                    key={s.id}
                    className="flex items-center justify-between p-3 bg-bg-tertiary border border-border rounded-lg"
                  >
                    <div>
                      <div className="text-sm text-text-primary font-medium">
                        {s.name}
                        <span className="ml-2 text-xs text-text-tertiary">{s.id}</span>
                      </div>
                      {s.prompt_template && (
                        <div className="text-xs text-text-tertiary mt-1 line-clamp-2">
                          {s.prompt_template}
                        </div>
                      )}
                    </div>
                    <label className="flex items-center gap-2 cursor-pointer">
                      <span className="text-xs text-text-secondary">
                        {s.enabled ? '启用' : '停用'}
                      </span>
                      <input
                        type="checkbox"
                        checked={s.enabled}
                        onChange={() => toggleDictSource(s.id)}
                        className="accent-primary"
                      />
                    </label>
                  </div>
                ))}
              </div>
            )}

            {sourcesError && (
              <p className="text-xs text-red-500 mb-2 bg-bg-tertiary p-2 rounded">
                {sourcesError}
              </p>
            )}
            {sourcesSaved && (
              <p className="text-xs text-primary mb-2 bg-bg-tertiary p-2 rounded">
                已保存
              </p>
            )}

            <div className="flex justify-end gap-2 mt-2">
              <button
                onClick={() => {
                  setShowSourcesDialog(false);
                  void loadDictSources();
                }}
                className="px-4 py-1.5 text-sm text-text-secondary hover:text-text-primary flex items-center gap-1.5"
              >
                <RefreshCw size={14} />
                刷新
              </button>
              <button
                onClick={() => setShowSourcesDialog(false)}
                className="px-4 py-1.5 text-sm text-text-secondary hover:text-text-primary"
              >
                关闭
              </button>
              <button
                onClick={handleSaveSources}
                disabled={isSavingSources || isSourcesLoading}
                className="px-4 py-1.5 text-sm bg-primary text-primary-fg rounded hover:bg-primary-hover disabled:opacity-50 flex items-center gap-1.5"
              >
                {isSavingSources ? (
                  <>
                    <Loader2 size={14} className="animate-spin" />
                    保存中...
                  </>
                ) : (
                  '保存'
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function ResultCard({
  result,
  onPlayAudio,
  onSpeak,
}: {
  result: ComprehensiveEntry;
  onPlayAudio: (url: string) => void;
  onSpeak: (text: string) => void;
}) {
  const { t } = useI18n();
  const primaryPhonetic = result.phonetics.find((p) => p.text);
  const [collected, setCollected] = useState(false);
  const [collectMsg, setCollectMsg] = useState<string | null>(null);

  const handleCollect = async () => {
    try {
      const { report } = await saveAndCollect({
        word: result.word,
        translation: result.chineseTranslation || '',
        fromLang: 'en',
        toLang: 'zh',
      });
      setCollected(true);
      setCollectMsg(summarizeReport(report));
    } catch (err) {
      console.error('Failed to add to wordbook:', err);
      setCollectMsg(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="space-y-4">
      <div className="bg-bg-secondary border border-border rounded-lg p-6 animate-fadeIn">
        <div className="flex items-start justify-between mb-3">
          <div>
            <h2 className="text-xl font-semibold text-text-primary mb-2 tracking-tight">
              {result.word}
            </h2>
            <div className="flex items-center gap-3 flex-wrap">
              {primaryPhonetic?.text && (
                <span className="text-text-secondary">/{primaryPhonetic.text}/</span>
              )}
              {/* 美音按钮 */}
              {result.usAudioUrl && (
                <button
                  onClick={() => onPlayAudio(result.usAudioUrl!)}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-bg-tertiary text-primary rounded hover:bg-bg-tertiary"
                  title="美式发音"
                >
                  <Volume2 size={12} /> 美音
                </button>
              )}
              {/* 英音按钮 */}
              {result.ukAudioUrl && (
                <button
                  onClick={() => onPlayAudio(result.ukAudioUrl!)}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-green-50 text-green-600 rounded hover:bg-green-100"
                  title="英式发音"
                >
                  <Volume2 size={12} /> 英音
                </button>
              )}
              {/* TTS 兜底发音（无词典音频时） */}
              {!result.usAudioUrl && !result.ukAudioUrl && (
                <button
                  onClick={() => onSpeak(result.word)}
                  className="flex items-center gap-1 px-2 py-1 text-xs bg-bg-tertiary text-primary rounded hover:bg-bg-tertiary"
                  title="发音"
                >
                  <Volume2 size={12} /> 发音
                </button>
              )}
              {result.sources.map((s) => (
                <span key={s} className="text-xs px-2 py-0.5 bg-bg-tertiary text-primary rounded">
                  {s}
                </span>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => void handleCollect()}
              disabled={collected}
              className={`flex items-center gap-1 px-3 py-1.5 text-xs rounded transition-colors ${
                collected
                  ? 'bg-bg-tertiary text-primary border border-primary'
                  : 'bg-bg-tertiary text-text-secondary hover:text-primary border border-border'
              }`}
              title={collected ? t('translator.collectedWithExport') : t('translator.collectToWordbook')}
            >
              {collected ? t('translator.collected') : t('translator.collect')}
            </button>
            <button
              onClick={() => navigator.clipboard.writeText(result.word)}
              className="p-2 hover:bg-bg-tertiary rounded"
              title="复制"
            >
              <Copy size={16} className="text-text-secondary" />
            </button>
          </div>
        </div>
        {collectMsg && (
          <p className="text-xs text-text-secondary mb-2 whitespace-pre-wrap">{collectMsg}</p>
        )}

        {/* 中文释义（ECDICT） */}
        {result.chineseTranslation && (
          <div className="mb-4">
            <h3 className="text-sm font-semibold text-primary mb-2 flex items-center gap-1.5">
              <Database size={14} /> 中文释义
            </h3>
            <p className="text-text-primary leading-relaxed bg-bg-primary p-3 rounded border border-border">
              {result.chineseTranslation}
            </p>
          </div>
        )}

        {/* 英文释义（ECDICT） */}
        {result.englishDefinitions.length > 0 && (
          <div className="mb-4">
            <h3 className="text-sm font-semibold text-primary mb-2">英文释义</h3>
            <ul className="space-y-1">
              {result.englishDefinitions.map((d, i) => (
                <li key={i} className="text-sm text-text-primary pl-3 border-l-2 border-border">
                  {d}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      {/* Oxford 权威释义 */}
      {result.oxfordDefinition && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <h3 className="text-sm font-semibold text-amber-600 mb-2 flex items-center gap-1.5">
            <BookOpen size={14} /> Oxford 释义
          </h3>
          <p className="text-sm text-text-primary leading-relaxed whitespace-pre-wrap">
            {result.oxfordDefinition}
          </p>
        </div>
      )}

      {/* 柯林斯词典（权威英英释义 + 双语例句） */}
      {result.collinsEntries.length > 0 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <h3 className="text-sm font-semibold text-orange-600 mb-3 flex items-center gap-1.5">
            <BookOpen size={14} /> 柯林斯词典
          </h3>
          <div className="space-y-4">
            {result.collinsEntries.map((ce, i) => (
              <div key={i}>
                <div className="flex items-center gap-2 mb-1.5">
                  <span className="text-xs px-1.5 py-0.5 bg-orange-100 text-orange-700 rounded font-mono">
                    {ce.pos}
                  </span>
                  {ce.posCn && <span className="text-xs text-text-secondary">{ce.posCn}</span>}
                </div>
                <p className="text-sm text-text-primary leading-relaxed mb-2">{ce.englishDef}</p>
                {ce.examples.length > 0 && (
                  <div className="space-y-1.5 ml-2">
                    {ce.examples.map((ex, j) => (
                      <div key={j} className="pl-3 border-l-2 border-orange-200">
                        <p className="text-xs text-text-primary italic">{ex.en}</p>
                        <p className="text-xs text-text-secondary">{ex.zh}</p>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* DictionaryAPI.dev 在线释义 */}
      {result.onlineMeanings.length > 0 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <h3 className="text-sm font-semibold text-green-600 mb-3 flex items-center gap-1.5">
            <Globe size={14} /> 在线释义（含例句）
          </h3>
          {result.onlineMeanings.map((m, mi) => (
            <div key={mi} className="mb-4 last:mb-0">
              <h4 className="text-xs font-semibold text-primary mb-2">{m.partOfSpeech}</h4>
              <div className="space-y-2">
                {m.definitions.map((d, di) => (
                  <div key={di} className="pl-3 border-l-2 border-border">
                    <p className="text-sm text-text-primary">{d.definition}</p>
                    {d.example && (
                      <p className="text-xs text-text-secondary italic mt-0.5">例: {d.example}</p>
                    )}
                    {d.synonyms.length > 0 && (
                      <p className="text-xs text-text-tertiary mt-0.5">
                        同义: {d.synonyms.join(', ')}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* 有道双语例句 */}
      {result.examples.length > 0 && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <h3 className="text-sm font-semibold text-neutral-500 mb-3 flex items-center gap-1.5">
            <BookOpen size={14} /> 双语例句
          </h3>
          <div className="space-y-3">
            {result.examples.map((ex, i) => (
              <div key={i} className="pl-3 border-l-2 border-border">
                <p className="text-sm text-text-primary">{ex.en}</p>
                <p className="text-xs text-text-secondary mt-0.5">{ex.zh}</p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* GPT4 词根分析 */}
      {result.gptAnalysis && (
        <div className="bg-bg-secondary border border-border rounded-lg p-5">
          <h3 className="text-sm font-semibold text-primary mb-2 flex items-center gap-1.5">
            <Sparkles size={14} /> AI 词根分析
          </h3>
          <div className="text-sm text-text-primary leading-relaxed whitespace-pre-wrap">
            {result.gptAnalysis}
          </div>
        </div>
      )}
    </div>
  );
}

export default DictionarySearch;
