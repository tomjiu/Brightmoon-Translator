// CoreVocabularyList - 核心词库列表组件

import { useState } from 'react';
import type { ChangeEvent } from 'react';
import { useCoreVocabulary, useSearchCoreVocabulary } from '../../hooks/useVocabulary';
import type { CoreVocabEntry } from '../../services/vocabulary';

interface CoreVocabularyListProps {
  onSelectWord?: (word: string) => void;
  className?: string;
}

export function CoreVocabularyList({ onSelectWord, className = '' }: CoreVocabularyListProps) {
  const [page, setPage] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');
  const pageSize = 50;

  // 如果有搜索，使用搜索 Hook，否则使用分页 Hook
  const { data: searchResults, isLoading: isSearching } = useSearchCoreVocabulary(
    searchQuery,
    20,
  );
  const { data: pagedResults, isLoading: isPaging } = useCoreVocabulary(page * pageSize, pageSize);

  const isLoading = searchQuery ? isSearching : isPaging;
  const entries = searchQuery ? searchResults : pagedResults;

  const handleWordClick = (word: string) => {
    onSelectWord?.(word);
  };

  const handleSearch = (e: ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(e.target.value);
    setPage(0);
  };

  return (
    <div className={`flex flex-col h-full ${className}`}>
      {/* 搜索栏 */}
      <div className="p-4 border-b">
        <input
          type="text"
          placeholder="搜索单词..."
          value={searchQuery}
          onChange={handleSearch}
          className="w-full px-4 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>

      {/* 词库列表 */}
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-32">
            <div className="text-gray-500">加载中...</div>
          </div>
        ) : entries && entries.length > 0 ? (
          <div className="divide-y">
            {entries.map((entry) => (
              <VocabEntryRow key={entry.word} entry={entry} onClick={handleWordClick} />
            ))}
          </div>
        ) : (
          <div className="flex items-center justify-center h-32">
            <div className="text-gray-500">
              {searchQuery ? '未找到匹配的单词' : '暂无数据'}
            </div>
          </div>
        )}
      </div>

      {/* 分页控制 */}
      {!searchQuery && entries && entries.length > 0 && (
        <div className="p-4 border-t flex items-center justify-between">
          <button
            onClick={() => setPage((p) => Math.max(0, p - 1))}
            disabled={page === 0}
            className="px-4 py-2 bg-gray-100 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-200"
          >
            上一页
          </button>
          <span className="text-sm text-gray-600">
            第 {page + 1} 页 (词频排名 {page * pageSize + 1} - {(page + 1) * pageSize})
          </span>
          <button
            onClick={() => setPage((p) => p + 1)}
            disabled={entries.length < pageSize}
            className="px-4 py-2 bg-gray-100 rounded-lg disabled:opacity-50 disabled:cursor-not-allowed hover:bg-gray-200"
          >
            下一页
          </button>
        </div>
      )}
    </div>
  );
}

interface VocabEntryRowProps {
  entry: CoreVocabEntry;
  onClick: (word: string) => void;
}

function VocabEntryRow({ entry, onClick }: VocabEntryRowProps) {
  return (
    <div
      className="p-4 hover:bg-gray-50 cursor-pointer transition-colors"
      onClick={() => onClick(entry.word)}
    >
      <div className="flex items-center justify-between">
        <div className="flex-1">
          <div className="flex items-center gap-3">
            <span className="text-lg font-semibold">{entry.word}</span>
            <span className="text-xs text-gray-500">#{entry.frequency_rank}</span>
            {entry.collins && entry.collins > 0 && (
              <span className="text-xs px-2 py-1 bg-blue-100 text-blue-700 rounded">
                ★{entry.collins}
              </span>
            )}
            {entry.oxford && (
              <span className="text-xs px-2 py-1 bg-green-100 text-green-700 rounded">
                Oxford 3000
              </span>
            )}
          </div>
          {entry.tag && (
            <div className="mt-1 flex gap-1">
              {entry.tag.split(' ').map((tag) => (
                <span
                  key={tag}
                  className="text-xs px-2 py-0.5 bg-gray-100 text-gray-600 rounded"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
        {entry.frq && (
          <div className="text-sm text-gray-500">词频: {entry.frq}</div>
        )}
      </div>
    </div>
  );
}
