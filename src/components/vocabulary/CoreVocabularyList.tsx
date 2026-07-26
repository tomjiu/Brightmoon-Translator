// CoreVocabularyList - 核心词库列表组件

import { useState, type ChangeEvent } from 'react';
import { useCoreVocabulary, useSearchCoreVocabulary } from '../../hooks/useVocabulary';
import type { CoreVocabEntry } from '../../services/vocabulary';
import { Search, ChevronLeft, ChevronRight } from 'lucide-react';

interface CoreVocabularyListProps {
  onSelectWord?: (word: string) => void;
  className?: string;
}

export function CoreVocabularyList({ onSelectWord, className = '' }: CoreVocabularyListProps) {
  const [page, setPage] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');
  const pageSize = 50;

  const { data: searchResults, isLoading: isSearching } = useSearchCoreVocabulary(searchQuery, 20);
  const { data: pagedResults, isLoading: isPaging } = useCoreVocabulary(page * pageSize, pageSize);

  const isLoading = searchQuery ? isSearching : isPaging;
  const entries = searchQuery ? searchResults : pagedResults;

  const handleSearch = (e: ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
    setPage(0);
  };

  return (
    <div className={`flex flex-col h-full ${className}`}>
      {/* 搜索栏 */}
      <div className="p-3 border-b border-border">
        <div className="relative">
          <Search
            className="absolute left-3 top-1/2 -translate-y-1/2 text-text-tertiary"
            size={16}
          />
          <input
            type="text"
            placeholder="搜索单词..."
            value={searchQuery}
            onChange={handleSearch}
            className="w-full pl-9 pr-3 py-2 bg-bg-primary border border-border rounded-lg text-sm text-text-primary focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </div>
      </div>

      {/* 词库列表 */}
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-32">
            <div className="text-text-secondary text-sm">加载中...</div>
          </div>
        ) : entries && entries.length > 0 ? (
          <div>
            {entries.map((entry) => (
              <VocabEntryRow key={entry.word} entry={entry} onClick={(w) => onSelectWord?.(w)} />
            ))}
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-32 text-text-tertiary text-sm">
            {searchQuery ? (
              '未找到匹配的单词'
            ) : (
              <>
                <p>暂无数据</p>
                <p className="text-xs mt-1">请先在词典页面导入核心词库</p>
              </>
            )}
          </div>
        )}
      </div>

      {/* 分页控制 */}
      {!searchQuery && entries && entries.length > 0 && (
        <div className="p-3 border-t border-border flex items-center justify-between">
          <button
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            disabled={page === 0}
            className="flex items-center gap-1 px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary rounded-lg disabled:opacity-40 hover:text-text-primary"
          >
            <ChevronLeft size={14} /> 上一页
          </button>
          <span className="text-xs text-text-secondary">
            #{page * pageSize + 1} - {(page + 1) * pageSize}
          </span>
          <button
            onClick={() => setPage((p) => p + 1)}
            disabled={entries.length < pageSize}
            className="flex items-center gap-1 px-3 py-1.5 text-xs bg-bg-tertiary text-text-secondary rounded-lg disabled:opacity-40 hover:text-text-primary"
          >
            下一页 <ChevronRight size={14} />
          </button>
        </div>
      )}
    </div>
  );
}

function VocabEntryRow({
  entry,
  onClick,
}: {
  entry: CoreVocabEntry;
  onClick: (w: string) => void;
}) {
  return (
    <div
      className="px-4 py-2.5 hover:bg-bg-tertiary cursor-pointer transition-colors border-b border-border/50"
      onClick={() => onClick(entry.word)}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-sm font-medium text-text-primary">{entry.word}</span>
          <span className="text-xs text-text-tertiary shrink-0">#{entry.frequency_rank}</span>
          {entry.collins && entry.collins > 0 && (
            <span className="text-xs px-1.5 py-0.5 bg-amber-100 text-amber-700 rounded shrink-0">
              ★{entry.collins}
            </span>
          )}
          {entry.oxford && (
            <span className="text-xs px-1.5 py-0.5 bg-green-100 text-green-700 rounded shrink-0">
              O3K
            </span>
          )}
        </div>
        {entry.frq && <span className="text-xs text-text-tertiary shrink-0">{entry.frq}</span>}
      </div>
      {entry.tag && (
        <div className="mt-0.5 flex gap-1 flex-wrap">
          {entry.tag
            .split(' ')
            .filter(Boolean)
            .map((tag) => (
              <span
                key={tag}
                className="text-xs px-1.5 py-0.5 bg-bg-tertiary text-text-tertiary rounded"
              >
                {tag}
              </span>
            ))}
        </div>
      )}
    </div>
  );
}
