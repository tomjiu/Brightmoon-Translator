import { useState, useEffect } from 'react';
import Card from '../components/Card';
import Badge from '../components/Badge';
import { Search, Star, BookOpen, Trash2, Download, Volume2 } from 'lucide-react';
import { useI18n } from '../i18n';

// 临时类型定义 - 后续可以移到types/index.ts
interface VocabularyEntry {
  id: string;
  word: string;
  phonetic?: string;
  translation: string;
  partOfSpeech?: string;
  examples: string[];
  addedAt: number;
  reviewCount: number;
  memoryStrength: number; // 0-100
  tags: string[];
  sourceContext?: {
    text: string;
    translation: string;
  };
}

export default function Dictionary() {
  const { t } = useI18n();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedEntry, setSelectedEntry] = useState<VocabularyEntry | null>(null);
  const [filterTag, setFilterTag] = useState<'all' | 'today' | 'review' | 'mastered'>('all');

  // 模拟数据 - 后续从Store获取
  const [entries] = useState<VocabularyEntry[]>([
    {
      id: '1',
      word: 'brilliant',
      phonetic: '/ˈbrɪliənt/',
      translation: '出色的；明亮的',
      partOfSpeech: 'adj.',
      examples: ['It was a brilliant performance.', 'The sun was brilliant in the sky.'],
      addedAt: Date.now() - 86400000 * 2,
      reviewCount: 3,
      memoryStrength: 80,
      tags: ['形容词', '高频'],
    },
    {
      id: '2',
      word: 'architecture',
      phonetic: '/ˈɑːkɪtektʃə/',
      translation: '建筑学；架构',
      partOfSpeech: 'n.',
      examples: [
        'Software architecture is important.',
        'The architecture of the building is stunning.',
      ],
      addedAt: Date.now() - 86400000,
      reviewCount: 2,
      memoryStrength: 60,
      tags: ['名词', '技术'],
    },
    {
      id: '3',
      word: 'abandon',
      phonetic: '/əˈbændən/',
      translation: '放弃；抛弃',
      partOfSpeech: 'v.',
      examples: ['They had to abandon the project.', 'Never abandon hope.'],
      addedAt: Date.now() - 86400000 * 5,
      reviewCount: 5,
      memoryStrength: 90,
      tags: ['动词', '常用'],
    },
  ]);

  const filteredEntries = entries.filter((entry) => {
    // 搜索过滤
    if (searchQuery && !entry.word.toLowerCase().includes(searchQuery.toLowerCase())) {
      return false;
    }

    // 标签过滤
    const now = Date.now();
    const dayMs = 86400000;
    switch (filterTag) {
      case 'today':
        return now - entry.addedAt < dayMs;
      case 'review':
        return entry.memoryStrength < 80;
      case 'mastered':
        return entry.memoryStrength >= 80;
      default:
        return true;
    }
  });

  useEffect(() => {
    if (filteredEntries.length > 0 && !selectedEntry) {
      setSelectedEntry(filteredEntries[0]);
    }
  }, [filteredEntries, selectedEntry]);

  const getMemoryStars = (strength: number) => {
    const stars = Math.round(strength / 20);
    return '⭐'.repeat(stars) + '☆'.repeat(5 - stars);
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp).toLocaleDateString('zh-CN', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });
  };

  return (
    <div className="flex h-full">
      {/* 左侧：生词列表 */}
      <div className="w-80 border-r border-border bg-bg-secondary flex flex-col">
        {/* 搜索栏 */}
        <div className="p-4 border-b border-border">
          <div className="relative">
            <Search
              size={18}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
            />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t('dictionary.search') || '搜索单词...'}
              className="w-full pl-10 pr-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 outline-none"
            />
          </div>
        </div>

        {/* 过滤器 */}
        <div className="p-4 border-b border-border space-y-2">
          <button
            onClick={() => setFilterTag('all')}
            className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
              filterTag === 'all'
                ? 'bg-primary/10 text-primary border border-primary/30'
                : 'text-text-secondary hover:bg-bg-tertiary'
            }`}
          >
            全部 ({entries.length})
          </button>
          <button
            onClick={() => setFilterTag('today')}
            className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
              filterTag === 'today'
                ? 'bg-primary/10 text-primary border border-primary/30'
                : 'text-text-secondary hover:bg-bg-tertiary'
            }`}
          >
            今日新增 ({entries.filter((e) => Date.now() - e.addedAt < 86400000).length})
          </button>
          <button
            onClick={() => setFilterTag('review')}
            className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
              filterTag === 'review'
                ? 'bg-primary/10 text-primary border border-primary/30'
                : 'text-text-secondary hover:bg-bg-tertiary'
            }`}
          >
            待复习 ({entries.filter((e) => e.memoryStrength < 80).length})
          </button>
          <button
            onClick={() => setFilterTag('mastered')}
            className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
              filterTag === 'mastered'
                ? 'bg-primary/10 text-primary border border-primary/30'
                : 'text-text-secondary hover:bg-bg-tertiary'
            }`}
          >
            已掌握 ({entries.filter((e) => e.memoryStrength >= 80).length})
          </button>
        </div>

        {/* 生词列表 */}
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {filteredEntries.map((entry) => (
            <button
              key={entry.id}
              onClick={() => setSelectedEntry(entry)}
              className={`w-full text-left p-3 rounded-lg transition-colors ${
                selectedEntry?.id === entry.id
                  ? 'bg-primary/10 border border-primary/30'
                  : 'hover:bg-bg-tertiary border border-transparent'
              }`}
            >
              <div className="flex items-center justify-between mb-1">
                <span className="font-medium text-text-primary">{entry.word}</span>
                <span className="text-xs">{getMemoryStars(entry.memoryStrength)}</span>
              </div>
              <p className="text-xs text-text-secondary truncate">{entry.translation}</p>
            </button>
          ))}
        </div>
      </div>

      {/* 右侧：词汇详情 */}
      <div className="flex-1 overflow-y-auto">
        {selectedEntry ? (
          <div className="max-w-3xl mx-auto p-8 space-y-6">
            {/* 单词标题 */}
            <div>
              <div className="flex items-start justify-between mb-2">
                <div>
                  <h1 className="text-3xl font-bold text-text-primary mb-2">
                    {selectedEntry.word}
                  </h1>
                  {selectedEntry.phonetic && (
                    <p className="text-lg text-text-secondary mb-2">{selectedEntry.phonetic}</p>
                  )}
                  {selectedEntry.partOfSpeech && (
                    <Badge variant="info">{selectedEntry.partOfSpeech}</Badge>
                  )}
                </div>
                <button
                  className="p-2 rounded-lg hover:bg-bg-secondary transition-colors text-text-secondary hover:text-primary"
                  title="发音"
                >
                  <Volume2 size={20} />
                </button>
              </div>
            </div>

            {/* 翻译 */}
            <Card title="释义">
              <p className="text-lg text-text-primary">{selectedEntry.translation}</p>
            </Card>

            {/* 例句 */}
            {selectedEntry.examples.length > 0 && (
              <Card title="例句">
                <div className="space-y-3">
                  {selectedEntry.examples.map((example, idx) => (
                    <div key={idx} className="p-3 bg-bg-secondary rounded-lg">
                      <p className="text-sm text-text-primary">{example}</p>
                    </div>
                  ))}
                </div>
              </Card>
            )}

            {/* 学习记录 */}
            <Card title="学习记录">
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <p className="text-xs text-text-secondary mb-1">首次添加</p>
                  <p className="text-sm text-text-primary">{formatDate(selectedEntry.addedAt)}</p>
                </div>
                <div>
                  <p className="text-xs text-text-secondary mb-1">复习次数</p>
                  <p className="text-sm text-text-primary">{selectedEntry.reviewCount} 次</p>
                </div>
                <div className="col-span-2">
                  <p className="text-xs text-text-secondary mb-2">记忆强度</p>
                  <div className="flex items-center gap-3">
                    <div className="flex-1 h-2 bg-bg-tertiary rounded-full overflow-hidden">
                      <div
                        className="h-full bg-primary transition-all"
                        style={{ width: `${selectedEntry.memoryStrength}%` }}
                      />
                    </div>
                    <span className="text-sm font-medium text-primary">
                      {selectedEntry.memoryStrength}%
                    </span>
                  </div>
                  <p className="text-xs text-text-secondary mt-1">
                    {getMemoryStars(selectedEntry.memoryStrength)}
                  </p>
                </div>
              </div>
            </Card>

            {/* 标签 */}
            {selectedEntry.tags.length > 0 && (
              <Card title="标签">
                <div className="flex flex-wrap gap-2">
                  {selectedEntry.tags.map((tag, idx) => (
                    <Badge key={idx} variant="default">
                      {tag}
                    </Badge>
                  ))}
                </div>
              </Card>
            )}

            {/* 操作按钮 */}
            <div className="flex gap-3">
              <button className="flex-1 px-4 py-2.5 bg-primary text-white rounded-lg hover:bg-primary/90 transition-colors flex items-center justify-center gap-2">
                <Star size={18} />
                标记为已掌握
              </button>
              <button className="px-4 py-2.5 bg-bg-secondary text-text-primary rounded-lg hover:bg-bg-tertiary transition-colors border border-border flex items-center gap-2">
                <Download size={18} />
                导出
              </button>
              <button className="px-4 py-2.5 bg-red-500/10 text-red-600 dark:text-red-400 rounded-lg hover:bg-red-500/20 transition-colors border border-red-500/30 flex items-center gap-2">
                <Trash2 size={18} />
                删除
              </button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-center h-full text-text-secondary">
            <div className="text-center">
              <BookOpen size={48} className="mx-auto mb-4 opacity-50" />
              <p>选择一个单词查看详情</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
