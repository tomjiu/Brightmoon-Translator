// DictionarySearch - 真正的词典查询页面

import { useState } from 'react';
import { Search, Volume2, BookMarked, Copy, ExternalLink } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

interface DictionaryResult {
  word: string;
  phonetic?: string;
  definitions: Definition[];
  collins?: number;
  oxford?: number;
  bnc?: number;
  frq?: number;
  exchange?: string;
  tag?: string;
  translation?: string;
}

interface Definition {
  pos: string; // part of speech
  def: string; // definition
}

function DictionarySearch() {
  const [searchQuery, setSearchQuery] = useState('');
  const [results, setResults] = useState<DictionaryResult[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSearch = async () => {
    if (!searchQuery.trim()) return;

    setIsLoading(true);
    setError(null);

    try {
      const data = await invoke<DictionaryResult[]>('lookup_dictionary', {
        text: searchQuery.trim(),
      });
      setResults(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : '查询失败');
      setResults([]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch();
    }
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
          <div className="flex gap-2">
            <div className="flex-1 relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-text-tertiary" size={20} />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyPress={handleKeyPress}
                placeholder="输入单词或短语查询..."
                className="w-full pl-10 pr-4 py-3 bg-bg-primary border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary text-text-primary"
              />
            </div>
            <button
              onClick={handleSearch}
              disabled={isLoading || !searchQuery.trim()}
              className="px-6 py-3 bg-primary text-white rounded-lg hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed font-medium"
            >
              {isLoading ? '查询中...' : '查询'}
            </button>
          </div>
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

          {results.length === 0 && !isLoading && !error && searchQuery && (
            <div className="text-center text-text-secondary py-12">
              <BookMarked size={48} className="mx-auto mb-4 opacity-50" />
              <p>未找到相关结果</p>
            </div>
          )}

          {results.length === 0 && !isLoading && !error && !searchQuery && (
            <div className="text-center text-text-secondary py-12">
              <Search size={48} className="mx-auto mb-4 opacity-50" />
              <p>输入单词或短语开始查询</p>
              <p className="text-sm mt-2">支持英汉、汉英双向查询</p>
            </div>
          )}

          {results.map((result, index) => (
            <DictionaryResultCard key={index} result={result} onCopy={copyToClipboard} />
          ))}
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
  return (
    <div className="bg-bg-secondary border border-border rounded-lg p-6 mb-4">
      {/* Word Header */}
      <div className="flex items-start justify-between mb-4">
        <div>
          <h2 className="text-3xl font-bold text-text-primary mb-2">{result.word}</h2>
          <div className="flex items-center gap-4">
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
                ★{result.collins} Collins
              </span>
            )}
            {result.oxford && result.oxford > 0 && (
              <span className="text-xs px-2 py-1 bg-green-100 text-green-700 rounded">
                Oxford 3000
              </span>
            )}
          </div>
        </div>

        <div className="flex gap-2">
          <button
            onClick={() => onCopy(result.word)}
            className="p-2 hover:bg-bg-tertiary rounded transition-colors"
            title="复制"
          >
            <Copy size={16} className="text-text-secondary" />
          </button>
        </div>
      </div>

      {/* Translation */}
      {result.translation && (
        <div className="mb-4 pb-4 border-b border-border">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">翻译</h3>
          <p className="text-text-primary">{result.translation}</p>
        </div>
      )}

      {/* Definitions */}
      {result.definitions && result.definitions.length > 0 && (
        <div className="mb-4">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">释义</h3>
          <div className="space-y-2">
            {result.definitions.map((def, i) => (
              <div key={i} className="flex gap-3">
                <span className="text-primary font-medium min-w-[60px]">{def.pos}</span>
                <span className="text-text-primary">{def.def}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Word Forms */}
      {result.exchange && (
        <div className="mb-4">
          <h3 className="text-sm font-semibold text-text-secondary mb-2">词形变化</h3>
          <p className="text-text-primary text-sm">{result.exchange}</p>
        </div>
      )}

      {/* Tags */}
      {result.tag && (
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
      )}

      {/* Frequency Info */}
      {(result.frq || result.bnc) && (
        <div className="mt-4 pt-4 border-t border-border flex gap-4 text-sm text-text-secondary">
          {result.frq && <span>词频: {result.frq}</span>}
          {result.bnc && <span>BNC: {result.bnc}</span>}
        </div>
      )}
    </div>
  );
}

export default DictionarySearch;
