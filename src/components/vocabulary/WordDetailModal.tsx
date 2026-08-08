import { useEffect, useState, type FC } from 'react';
import { X, History, Edit3, Link2, BookOpen, Lightbulb, Save, BarChart3, Volume2 } from 'lucide-react';
import {
  getWordHistory,
  getFsrsTimeline,
  updateAiContent,
  getRootGraph,
  getCorpusExamples,
  getWordEtymology,
  type WordHistory,
  type FsrsTimeline,
  type RootGraph,
  type AiContent,
} from '../../services/wordDetail';
import { getCardPatchHistory, type PatchHistoryEntry } from '../../services/vocabulary';
import { speakText, stopSpeaking } from '../../services/tts';
import { detectSpeakLang } from '../../utils/speech';
import { useToastStore } from '../../stores/toastStore';

interface WordDetailModalProps {
  word: string;
  onClose: () => void;
}

export const WordDetailModal: FC<WordDetailModalProps> = ({ word, onClose }) => {
  const addToast = useToastStore((s) => s.addToast);
  const [history, setHistory] = useState<WordHistory[]>([]);
  const [timeline, setTimeline] = useState<FsrsTimeline[]>([]);
  const [rootGraph, setRootGraph] = useState<RootGraph | null>(null);
  const [examples, setExamples] = useState<string[]>([]);
  const [etymology, setEtymology] = useState<string>('');
  const [aiContent, setAiContent] = useState<AiContent>({});
  const [editingAi, setEditingAi] = useState(false);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<
    'history' | 'timeline' | 'related' | 'examples' | 'patches'
  >('history');
  const [patches, setPatches] = useState<PatchHistoryEntry[]>([]);

  useEffect(() => {
    loadWordDetail();
  }, [word]);

  const loadWordDetail = async () => {
    try {
      setLoading(true);
      const [historyData, timelineData, graphData, examplesData, etymologyData] =
        await Promise.all([
          getWordHistory(word),
          getFsrsTimeline(word),
          getRootGraph(word),
          getCorpusExamples(word, 5),
          getWordEtymology(word),
        ]);

      setHistory(historyData);
      setTimeline(timelineData);
      setRootGraph(graphData);
      setExamples(examplesData);
      setEtymology(etymologyData);

      // T8: 并行加载 AI 增强 Patch 历史(答错触发过才有数据)
      getCardPatchHistory(word)
        .then(setPatches)
        .catch(() => setPatches([]));
    } catch (error) {
      console.error('加载单词详情失败:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleSaveAiContent = async () => {
    try {
      await updateAiContent(word, aiContent);
      setEditingAi(false);
      addToast({ type: 'success', message: 'AI 内容已更新！', duration: 3000 });
    } catch (error) {
      console.error('更新AI内容失败:', error);
      addToast({ type: 'error', message: '更新失败，请重试', duration: 3000 });
    }
  };

  const speakWord = async (text: string) => {
    try {
      await speakText(text, detectSpeakLang(text));
    } catch (error) {
      console.error('发音失败:', error);
      addToast({ type: 'error', message: '发音失败，请检查网络', duration: 3000 });
    }
  };

  useEffect(() => {
    return () => stopSpeaking();
  }, []);

  const getEventIcon = (eventType: string) => {
    switch (eventType) {
      case 'word_imported':
        return '📥';
      case 'fsrs_updated':
        return '📝';
      case 'ai_content_generated':
        return '🤖';
      default:
        return '•';
    }
  };

  const getEventLabel = (eventType: string) => {
    switch (eventType) {
      case 'word_imported':
        return '导入单词';
      case 'fsrs_updated':
        return '复习记录';
      case 'ai_content_generated':
        return 'AI内容生成';
      default:
        return eventType;
    }
  };

  const getRatingColor = (rating?: string) => {
    switch (rating) {
      case 'again':
        return 'text-red-400';
      case 'hard':
        return 'text-yellow-400';
      case 'good':
        return 'text-green-400';
      case 'easy':
        return 'text-primary';
      default:
        return 'text-gray-400';
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-gray-900 rounded-lg w-full max-w-4xl max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-700">
          <div className="flex items-center gap-3">
            <div>
              <h2 className="text-2xl font-bold flex items-center gap-2">
                {word}
                <button
                  onClick={() => void speakWord(word)}
                  className="p-1.5 rounded-lg hover:bg-gray-700 transition-colors"
                  title="发音"
                >
                  <Volume2 className="w-5 h-5 text-primary" />
                </button>
              </h2>
              <p className="text-sm text-gray-400 mt-1">单词详情与学习历史</p>
            </div>
          </div>
          <button onClick={onClose} className="p-2 hover:bg-gray-800 rounded-lg transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        {loading ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-gray-400">加载中...</div>
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto p-6 space-y-6">
            {/* Etymology */}
            <div className="bg-gray-800 rounded-lg p-4">
              <div className="flex items-center gap-2 mb-3">
                <Lightbulb className="w-5 h-5 text-yellow-400" />
                <h3 className="font-semibold">词根词缀分析</h3>
              </div>
              <p className="text-gray-300 whitespace-pre-line">{etymology}</p>
            </div>

            {/* AI Content Editor */}
            <div className="bg-gray-800 rounded-lg p-4">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <Edit3 className="w-5 h-5 text-primary" />
                  <h3 className="font-semibold">AI 助记内容</h3>
                </div>
                {editingAi ? (
                  <button
                    onClick={handleSaveAiContent}
                    className="flex items-center gap-2 px-3 py-1.5 bg-primary hover:bg-primary-hover rounded-lg text-sm transition-colors"
                  >
                    <Save className="w-4 h-4" />
                    保存
                  </button>
                ) : (
                  <button
                    onClick={() => setEditingAi(true)}
                    className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded-lg text-sm transition-colors"
                  >
                    编辑
                  </button>
                )}
              </div>
              {editingAi ? (
                <div className="space-y-3">
                  <textarea
                    placeholder="助记法"
                    value={aiContent.mnemonics || ''}
                    onChange={(e) => setAiContent({ ...aiContent, mnemonics: e.target.value })}
                    className="w-full px-3 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/40 resize-none"
                    rows={3}
                  />
                  <textarea
                    placeholder="学习技巧"
                    value={aiContent.tips || ''}
                    onChange={(e) => setAiContent({ ...aiContent, tips: e.target.value })}
                    className="w-full px-3 py-2 bg-gray-700 rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/40 resize-none"
                    rows={2}
                  />
                </div>
              ) : (
                <div className="space-y-2 text-gray-300">
                  {aiContent.mnemonics && <p>{aiContent.mnemonics}</p>}
                  {aiContent.tips && <p className="text-sm text-gray-400">{aiContent.tips}</p>}
                  {!aiContent.mnemonics && !aiContent.tips && (
                    <p className="text-gray-500">暂无AI内容</p>
                  )}
                </div>
              )}
            </div>

            {/* Tabs */}
            <div className="flex gap-2 border-b border-gray-700">
              <button
                onClick={() => setActiveTab('history')}
                className={`flex items-center gap-2 px-4 py-2 border-b-2 transition-colors ${
                  activeTab === 'history'
                    ? 'border-border text-primary'
                    : 'border-transparent text-gray-400 hover:text-gray-300'
                }`}
              >
                <History className="w-4 h-4" />
                学习历史
              </button>
              <button
                onClick={() => setActiveTab('timeline')}
                className={`flex items-center gap-2 px-4 py-2 border-b-2 transition-colors ${
                  activeTab === 'timeline'
                    ? 'border-border text-primary'
                    : 'border-transparent text-gray-400 hover:text-gray-300'
                }`}
              >
                <BarChart3 className="w-4 h-4" />
                参数曲线
              </button>
              <button
                onClick={() => setActiveTab('related')}
                className={`flex items-center gap-2 px-4 py-2 border-b-2 transition-colors ${
                  activeTab === 'related'
                    ? 'border-border text-primary'
                    : 'border-transparent text-gray-400 hover:text-gray-300'
                }`}
              >
                <Link2 className="w-4 h-4" />
                相关词汇
              </button>
              <button
                onClick={() => setActiveTab('examples')}
                className={`flex items-center gap-2 px-4 py-2 border-b-2 transition-colors ${
                  activeTab === 'examples'
                    ? 'border-border text-primary'
                    : 'border-transparent text-gray-400 hover:text-gray-300'
                }`}
              >
                <BookOpen className="w-4 h-4" />
                语料例句
              </button>
              <button
                onClick={() => setActiveTab('patches')}
                className={`flex items-center gap-2 px-4 py-2 border-b-2 transition-colors ${
                  activeTab === 'patches'
                    ? 'border-border text-primary'
                    : 'border-transparent text-gray-400 hover:text-gray-300'
                }`}
              >
                <Edit3 className="w-4 h-4" />
                AI 增强
              </button>
            </div>

            {/* Tab Content */}
            <div className="bg-gray-800 rounded-lg p-4">
              {activeTab === 'history' && (
                <div className="space-y-3">
                  {history.length === 0 ? (
                    <p className="text-gray-400 text-center py-8">暂无学习历史</p>
                  ) : (
                    history.map((event, idx) => (
                      <div
                        key={idx}
                        className="flex items-start gap-3 p-3 bg-gray-700/50 rounded-lg"
                      >
                        <span className="text-2xl">{getEventIcon(event.eventType)}</span>
                        <div className="flex-1">
                          <div className="flex items-center justify-between mb-1">
                            <span className="font-semibold">{getEventLabel(event.eventType)}</span>
                            <span className="text-xs text-gray-400">
                              {new Date(event.timestamp * 1000).toLocaleString()}
                            </span>
                          </div>
                          {event.rating && (
                            <div className="flex items-center gap-4 text-sm">
                              <span className={`font-medium ${getRatingColor(event.rating)}`}>
                                评分: {event.rating.toUpperCase()}
                              </span>
                              {event.difficulty && (
                                <span className="text-gray-400">
                                  难度: {event.difficulty.toFixed(2)}
                                </span>
                              )}
                              {event.stability && (
                                <span className="text-gray-400">
                                  稳定性: {event.stability.toFixed(2)}
                                </span>
                              )}
                            </div>
                          )}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              )}

              {activeTab === 'timeline' && (
                <div className="space-y-4">
                  {timeline.length === 0 ? (
                    <p className="text-gray-400 text-center py-8">暂无FSRS数据</p>
                  ) : (
                    <>
                      <div className="h-48 flex items-end gap-2">
                        {timeline.map((point, idx) => {
                          const maxDiff = Math.max(...timeline.map((p) => p.difficulty));
                          const height = (point.difficulty / maxDiff) * 100;
                          return (
                            <div
                              key={idx}
                              className="flex-1 flex flex-col items-center group relative"
                            >
                              <div
                                className="w-full bg-red-500 hover:bg-red-400 transition-colors rounded-t"
                                style={{ height: `${height}%` }}
                                title={`难度: ${point.difficulty.toFixed(2)}`}
                              />
                              <div className="absolute bottom-0 left-1/2 transform -translate-x-1/2 translate-y-full mt-2 px-2 py-1 bg-gray-900 rounded text-xs whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10">
                                {point.date}
                                <br />
                                难度: {point.difficulty.toFixed(2)}
                                <br />
                                稳定性: {point.stability.toFixed(2)}
                              </div>
                            </div>
                          );
                        })}
                      </div>
                      <div className="text-center text-sm text-gray-400">难度变化曲线（红色）</div>
                    </>
                  )}
                </div>
              )}

              {activeTab === 'related' && (
                <div>
                  <div className="flex items-center gap-2 mb-3">
                    <Lightbulb className="w-5 h-5 text-yellow-400" />
                    <h3 className="font-semibold">词根图谱</h3>
                    {rootGraph?.source === 'heuristic' && (
                      <span className="text-xs text-gray-500">（启发式匹配）</span>
                    )}
                  </div>

                  {!rootGraph || rootGraph.rootMates.length === 0 ? (
                    <p className="text-gray-500 text-sm">暂无同根词数据</p>
                  ) : (
                    <div>
                      <div className="flex items-center gap-2 mb-4">
                        <span className="px-3 py-1 bg-primary/20 text-primary rounded-lg font-semibold">
                          {word}
                        </span>
                        <span className="text-gray-500">← 词根:</span>
                        {rootGraph.roots.map((root, idx) => (
                          <span
                            key={idx}
                            className="px-2 py-1 bg-gray-700 rounded text-yellow-400 font-mono"
                          >
                            {root}
                          </span>
                        ))}
                      </div>
                      <div className="space-y-1.5">
                        {rootGraph.rootMates.map((mate) => (
                          <div key={mate.word} className="flex items-center gap-2 py-1">
                            <span className="w-28 font-medium text-blue-300 truncate">
                              {mate.word}
                            </span>
                            <span className="text-gray-400 text-xs truncate">
                              {mate.definition || '暂无释义'}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}

              {activeTab === 'examples' && (
                <div className="space-y-3">
                  {examples.length === 0 ? (
                    <p className="text-gray-400 text-center py-8">暂无例句</p>
                  ) : (
                    examples.map((example, idx) => (
                      <div
                        key={idx}
                        className="flex items-start justify-between gap-3 p-3 bg-gray-700/50 rounded-lg text-gray-300 group"
                      >
                        <span>{example}</span>
                        <button
                          onClick={() => void speakWord(example)}
                          className="p-1.5 rounded-lg hover:bg-gray-600 transition-colors shrink-0"
                          title="朗读例句"
                        >
                          <Volume2 className="w-4 h-4 text-gray-400 group-hover:text-primary" />
                        </button>
                      </div>
                    ))
                  )}
                </div>
              )}

              {activeTab === 'patches' && (
                <div className="space-y-3">
                  {patches.length === 0 ? (
                    <p className="text-gray-400 text-center py-8">暂无 AI 增强记录</p>
                  ) : (
                    patches.map((patch, idx) => (
                      <div
                        key={idx}
                        className="flex items-start gap-3 p-3 bg-gray-700/50 rounded-lg"
                      >
                        <Edit3 className="w-4 h-4 text-primary mt-1" />
                        <div className="flex-1">
                          <div className="flex items-center justify-between mb-1">
                            <span className="font-semibold text-sm">
                              v{patch.version} · {patch.field}
                            </span>
                            <span className="text-xs text-gray-400">
                              {new Date(patch.timestamp * 1000).toLocaleString()}
                            </span>
                          </div>
                          <span
                            className={`inline-block text-xs px-2 py-0.5 rounded mb-1 ${
                              patch.operation === 'replace'
                                ? 'bg-green-500/20 text-green-300'
                                : patch.operation === 'insert'
                                  ? 'bg-blue-500/20 text-blue-300'
                                  : 'bg-red-500/20 text-red-300'
                            }`}
                          >
                            {patch.operation}
                          </span>
                          {patch.reasoning && (
                            <p className="text-sm text-gray-400 mt-1">理由: {patch.reasoning}</p>
                          )}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
