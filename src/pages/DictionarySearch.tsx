// DictionarySearch - 真正的词典查询页面（带实时联想）

import { useState, useEffect, useRef } from 'react';
import { Search, Volume2, BookMarked, Copy, Loader2, X } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface DictionaryResult {
  word: string;
  phonetic?: string;
  definition?: string;
  translation?: string;
  pos?: string;
  collins?: number;
  oxford?: number;
  bnc?: number;
  frq?: number;
  exchange?: string;
  tag?: string;
}

function DictionarySearch() {
  const [searchQuery, setSearchQuery] = useState('');
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(-1);
  const [result, setResult] = useState<DictionaryResult | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSuggestionsLoading, setIsSuggestionsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // 实时联想搜索
  useEffect(() => {
    const fetchSuggestions = async () => {
      if (!searchQuery.trim() || searchQuery.length < 2) {
        setSuggestions([]);
        return;
      }

      setIsSuggestionsLoading(true);
      try {
        const data = await invoke<string[]>('search_word_suggestions', {
          query: searchQuery.trim(),
          limit: 10,
        });
        setSuggestions(data);
        setShowSuggestions(true);
        setSelectedIndex(-1);
      } catch (err) {
        console.error('Failed to fetch suggestions:', err);
        setSuggestions([]);
      } finally {
        setIsSuggestionsLoading(false);
      }
    };

    const debounce = setTimeout(fetchSuggestions, 200);
    return () => clearTimeout(debounce);
  }, [searchQuery]);

  // 点击外部关闭建议列表
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (searchRef.current && !searchRef.current.contains(event.target as Node)) {
        setShowSuggestions(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleLookup = async (word: string) => {
    if (!word.trim()) return;

    setIsLoading(true);
    setError(null);
    setShowSuggestions(false);

    try {
      const data = await invoke<DictionaryResult>('lookup_word_detail', {
        word: word.trim(),
      });
      setResult(data);
      setSearchQuery(data.word);
    } catch (err) {
      setError(err instanceof Error ? err.message : '查询失败');
      setResult(null);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (!showSuggestions || suggestions.length === 0) {
      if (e.key === 'Enter') {
        handleLookup(searchQuery);
      }
      return;
    }

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex((prev) =>
          prev < suggestions.length - 1 ? prev + 1 : prev
        );
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : -1));
        break;
      case 'Enter':
        e.preventDefault();
        if (selectedIndex >= 0 && selectedIndex < suggestions.length) {
          handleLookup(suggestions[selectedIndex]);
        } else {
          handleLookup(searchQuery);
        }
        break;
      case 'Escape':
        setShowSuggestions(false);
        break;
    }
  };

  const handleSuggestionClick = (word: string) => {
    handleLookup(word);
  };

  const handleClear = () => {
    setSearchQuery('');
    setResult(null);
    setError(null);
    setSuggestions([]);
    inputRef.current?.focus();
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
  };

  return (
    <div className="h-full flex flex-col bg-bg-primary">
      {/* Search Bar */}
      <div className="p-6 border-b border-border bg-bg-secondary">
        <div className="max-w-3xl mx-auto">
          <h1 className="text-2xl font-bold mb-4 text-text-primary">词典查询</h1>
          <div ref={searchRef} className="relative">
            <div className="flex gap-2">
              <div className="flex-1 relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-text-tertiary" size={20} />
                <input
                  ref={inputRef}
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  onKeyDown={handleKeyDown}
                  onFocus={() => suggestions.length > 0 && setShowSuggestions(true)}
                  placeholder="输入单词查询..."
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
                onClick={() => handleLookup(searchQuery)}
                disabled={isLoading || !searchQuery.trim()}
                className="px-6 py-3 bg-primary text-white rounded-lg hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
              >
                {isLoading ? <Loader2 className="animate-spin" size={20} /> : '查询'}
              </button>
            </div>

            {/* Suggestions Dropdown */}
            {showSuggestions && (suggestions.length > 0 || isSuggestionsLoading) && (
              <div className="absolute top-full left-0 right-16 mt-1 bg-bg-secondary border border-border rounded-lg shadow-lg max-h-80 overflow-y-auto z-50">
                {isSuggestionsLoading ? (
                  <div className="p-4 text-center text-text-secondary">
                    <Loader2 className="animate-spin inline-block" size={20} />
                  </div>
                ) : (
                  suggestions.map((word, index) => (
                    <button
                      key={word}
                      onClick={() => handleSuggestionClick(word)}
                      className={`w-full px-4 py-2 text-left hover:bg-bg-tertiary transition-colors ${
                        index === selectedIndex ? 'bg-bg-tertiary' : ''
                      }`}
                    >
                      <span className="text-text-primary font-medium">{word}</span>
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
          <p className="text-xs text-text-secondary mt-2">
            💡 提示：输入 2 个字母以上自动显示联想词汇
          </p>
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
            <div className="text-center text-text-secondary py-12">
              <Search size={48} className="mx-auto mb-4 opacity-50" />
              <p className="text-lg font-medium">输入单词开始查询</p>
              <p className="text-sm mt-2">支持英汉、汉英双向查询</p>
              <p className="text-sm mt-1">输入时自动显示联想词汇</p>
            </div>
          )}

          {result && <DictionaryResultCard result={result} onCopy={copyToClipboard} />}
        </div>
      </div>
    </div>
  );
}

interface DictionaryResultCardProps {
  result: DictionaryResult;
  onCopy: (text: string) => void;
}

function DictionaryResultCard({ result, onCopy }: DictionaryResultCardProps) {
  // 解析释义
  const definitions = result.definition
    ? result.definition.split('\\n').filter((d) => d.trim())
    : [];

  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-6 mb-4 animate-fadeIn">
      {/* Word Header */}
      <div className="flex items-start justify-between mb-4">
        <div>
          <h2 className="text-3xl font-bold text-text-primary mb-2">{result.word}</h2>
          <div className="flex items-center gap-4 flex-wrap">
            {result.phonetic && (
              <div className="flex items-center gap-2">
                <span className="text-text-secondary">/{result.phonetic}/</span>
                <button
                  className="p-1 hover:bg-bg-tertiary rounded transition-colors"
                  title="发音"
                >
                  <Volume2 size={16} className="text-primary" />
                </button>
              </div>
            )}
            {result.collins && result.collins > 0 && (
              <span className="text-xs px-2 py-1 bg-blue-100 text-blue-700 rounded">
                {'★'.repeat(result.collins)} Collins
              </span>
            )}
            {result.oxford && result.oxford > 0 && (
              <span className="text-xs px-2 py-1 bg-green-100 text-green-700 rounded">
                Oxford 3000
              </span>
            )}
          </div>
        </div>

        <button
          onClick={() => onCopy(result.word)}
          className="p-2 hover:bg-bg-tertiary rounded transition-colors"
          title="复制"
        >
          <Copy size={16} className="text-text-secondary" />
        </button>
      </div>

      {/* Translation */}
      {result.translation && (
        <div className="mb-4 pb-4 border-b border-border">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">翻译</h3>
          <p className="text-text-primary text-lg">{result.translation}</p>
        </div>
      )}

      {/* Definitions */}
      {definitions.length > 0 && (
        <div className="mb-4 pb-4 border-b border-border">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">详细释义</h3>
          <div className="space-y-2">
            {definitions.map((def, i) => (
              <div key={i} className="flex gap-2">
                <span className="text-primary font-medium min-w-[24px]">{i + 1}.</span>
                <span className="text-text-primary">{def}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Word Forms */}
      {result.exchange && (
        <div className="mb-4 pb-4 border-b border-border">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">词形变化</h3>
          <p className="text-text-primary text-sm">{result.exchange}</p>
        </div>
      )}

      {/* Tags */}
      {result.tag && (
        <div className="mb-4 pb-4 border-b border-border">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">标签</h3>
          <div className="flex gap-2 flex-wrap">
            {result.tag.split(' ').map((tag) => (
              <span
                key={tag}
                className="text-xs px-2 py-1 bg-bg-tertiary text-text-secondary rounded"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Frequency Info */}
      {(result.frq || result.bnc) && (
        <div className="flex gap-4 text-sm text-text-secondary">
          {result.frq && <span>词频: {result.frq}</span>}
          {result.bnc && <span>BNC: {result.bnc}</span>}
        </div>
      )}
    </div>
  );
}

export default DictionarySearch;
