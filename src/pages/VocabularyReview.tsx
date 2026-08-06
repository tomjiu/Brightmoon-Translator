// VocabularyReview - 完整的FSRS复习系统

import { useState, useEffect, useCallback } from 'react';
import { invokeOrThrow } from '../services/invoke';
import { useVocabularyStore } from '../stores/vocabularyStore';
import {
  RefreshCw,
  CheckCircle2,
  Clock,
  TrendingUp,
  Volume2,
  Sparkles,
  ArrowLeft,
  Trophy,
} from 'lucide-react';
import type { CardInfo, AiContent } from '../services/vocabulary';

interface ReviewSession {
  totalCards: number;
  reviewedCount: number;
  againCount: number;
  hardCount: number;
  goodCount: number;
  easyCount: number;
  startTime: number;
  correctRate: number;
}

interface WordDetailData {
  word: string;
  cardId: string;
  phonetic?: string;
  chineseTranslation?: string;
  englishDefinitions: string[];
  collinsEntries: CollinsEntry[];
  examples: BilingualExample[];
  usAudioUrl?: string;
  ukAudioUrl?: string;
  aiContent?: AiContent;
  imageUrl?: string;
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

export default function VocabularyReview() {
  const [dueCards, setDueCards] = useState<CardInfo[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [showAnswer, setShowAnswer] = useState(false);
  const [wordDetail, setWordDetail] = useState<WordDetailData | null>(null);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [session, setSession] = useState<ReviewSession>({
    totalCards: 0,
    reviewedCount: 0,
    againCount: 0,
    hardCount: 0,
    goodCount: 0,
    easyCount: 0,
    startTime: Date.now(),
    correctRate: 0,
  });
  const [showStats, setShowStats] = useState(false);

  const { startSession, endSession } = useVocabularyStore();

  // 加载待复习卡牌
  const loadDueCards = useCallback(async () => {
    setLoading(true);
    try {
      const cards = await invokeOrThrow<CardInfo[]>('get_due_cards');
      setDueCards(cards);
      setSession({
        totalCards: cards.length,
        reviewedCount: 0,
        againCount: 0,
        hardCount: 0,
        goodCount: 0,
        easyCount: 0,
        startTime: Date.now(),
        correctRate: 0,
      });

      if (cards.length > 0) {
        startSession();
        await loadWordDetail(cards[0].word);
      }
    } catch (error) {
      console.error('加载复习队列失败:', error);
    } finally {
      setLoading(false);
    }
  }, [startSession]);

  useEffect(() => {
    void loadDueCards();
  }, [loadDueCards]);

  // 键盘快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // 忽略输入框中的按键
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        return;
      }

      if (showStats) {
        // 统计页面：Enter 重新开始
        if (e.key === 'Enter') {
          handleRestart();
        }
        return;
      }

      if (!showAnswer) {
        // 未显示答案：空格/Enter 显示答案
        if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault();
          setShowAnswer(true);
        }
        return;
      }

      // 显示答案后：评分快捷键
      switch (e.key) {
        case '1':
          handleRate('Again');
          break;
        case '2':
          handleRate('Hard');
          break;
        case '3':
          handleRate('Good');
          break;
        case '4':
          handleRate('Easy');
          break;
        case 'ArrowLeft':
          handleRate('Again');
          break;
        case 'ArrowRight':
          handleRate('Easy');
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showAnswer, showStats, currentIndex, dueCards, submitting]);

  // 加载单词详情
  const loadWordDetail = async (word: string) => {
    try {
      const data = await invokeOrThrow<WordDetailData>('study_word', { word });
      setWordDetail(data);
    } catch (error) {
      console.error('加载单词详情失败:', error);
    }
  };

  // 提交评分
  const handleRate = async (rating: 'Again' | 'Hard' | 'Good' | 'Easy') => {
    if (!dueCards[currentIndex] || submitting) return;

    setSubmitting(true);
    try {
      await invokeOrThrow('submit_review', {
        cardId: dueCards[currentIndex].id,
        rating,
      });

      // 更新统计
      const newSession = { ...session };
      newSession.reviewedCount += 1;

      if (rating === 'Again') newSession.againCount += 1;
      else if (rating === 'Hard') newSession.hardCount += 1;
      else if (rating === 'Good') newSession.goodCount += 1;
      else if (rating === 'Easy') newSession.easyCount += 1;

      // 计算正确率（Hard/Good/Easy算正确）
      const correctCount = newSession.hardCount + newSession.goodCount + newSession.easyCount;
      newSession.correctRate = (correctCount / newSession.reviewedCount) * 100;

      setSession(newSession);

      // 移动到下一张
      const nextIndex = currentIndex + 1;
      if (nextIndex >= dueCards.length) {
        // 复习完成
        endSession();
        setShowStats(true);
      } else {
        setCurrentIndex(nextIndex);
        setShowAnswer(false);
        await loadWordDetail(dueCards[nextIndex].word);
      }
    } catch (error) {
      console.error('提交复习失败:', error);
    } finally {
      setSubmitting(false);
    }
  };

  // 播放音频
  const playAudio = (url: string) => {
    new Audio(url).play().catch((err: unknown) => {
      console.error(err);
    });
  };

  // 退出复习
  const handleExit = () => {
    if (session.reviewedCount > 0 && currentIndex < dueCards.length) {
      // eslint-disable-next-line no-alert -- destructive confirm; no dialog component available
      if (!confirm('复习尚未完成，确定要退出吗？')) return;
    }
    endSession();
    window.history.back();
  };

  // 重新开始
  const handleRestart = () => {
    setCurrentIndex(0);
    setShowAnswer(false);
    setShowStats(false);
    void loadDueCards();
  };

  // 加载中
  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center">
          <RefreshCw className="animate-spin mx-auto mb-2 text-primary" size={32} />
          <p className="text-text-secondary">加载复习队列...</p>
        </div>
      </div>
    );
  }

  // 没有待复习卡牌
  if (dueCards.length === 0) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-center max-w-md p-8">
          <CheckCircle2 className="mx-auto mb-4 text-green-500" size={64} />
          <h2 className="ui-stat text-text-primary mb-2">🎉 太棒了！</h2>
          <p className="text-text-secondary mb-4">目前没有需要复习的卡牌</p>
          <button
            onClick={handleExit}
            className="px-6 py-2 bg-primary text-primary-fg rounded-lg hover:bg-primary/90"
          >
            返回
          </button>
        </div>
      </div>
    );
  }

  // 复习完成统计
  if (showStats) {
    const duration = Math.floor((Date.now() - session.startTime) / 1000);
    const minutes = Math.floor(duration / 60);
    const seconds = duration % 60;
    const avgTime = Math.floor(duration / session.reviewedCount);

    return (
      <div className="h-full flex items-center justify-center bg-gradient-to-br from-green-50 to-neutral-50 dark:from-gray-900 dark:to-gray-800">
        <div className="max-w-2xl w-full p-8 text-center">
          <Trophy className="mx-auto mb-4 text-yellow-500" size={80} />
          <h1 className="ui-page-title mb-2">复习完成</h1>
          <p className="ui-page-desc mb-8">本轮复习已结束</p>

          {/* 统计卡片 */}
          <div className="grid grid-cols-2 gap-4 mb-8">
            <div className="p-6 ui-card">
              <div className="ui-stat text-primary mb-1">{session.reviewedCount}</div>
              <div className="ui-caption">复习卡牌数</div>
            </div>
            <div className="p-6 ui-card">
              <div className="ui-stat text-primary mb-1">{session.correctRate.toFixed(0)}%</div>
              <div className="text-sm text-text-secondary">正确率</div>
            </div>
            <div className="p-6 ui-card">
              <div className="text-4xl font-bold text-primary mb-1">
                {minutes}:{seconds.toString().padStart(2, '0')}
              </div>
              <div className="text-sm text-text-secondary">总用时</div>
            </div>
            <div className="p-6 ui-card">
              <div className="text-4xl font-bold text-primary mb-1">{avgTime}s</div>
              <div className="text-sm text-text-secondary">平均用时</div>
            </div>
          </div>

          {/* 评分分布 */}
          <div className="p-6 ui-card mb-8">
            <h3 className="text-sm font-semibold text-text-secondary mb-4">评分分布</h3>
            <div className="grid grid-cols-4 gap-3">
              <div className="text-center">
                <div className="ui-stat text-red-500 mb-1">{session.againCount}</div>
                <div className="text-xs text-text-tertiary">忘记</div>
              </div>
              <div className="text-center">
                <div className="ui-stat text-orange-500 mb-1">{session.hardCount}</div>
                <div className="text-xs text-text-tertiary">困难</div>
              </div>
              <div className="text-center">
                <div className="ui-stat text-green-500 mb-1">{session.goodCount}</div>
                <div className="text-xs text-text-tertiary">良好</div>
              </div>
              <div className="text-center">
                <div className="ui-stat text-primary mb-1">{session.easyCount}</div>
                <div className="text-xs text-text-tertiary">简单</div>
              </div>
            </div>
          </div>

          {/* 操作按钮 */}
          <div className="flex gap-3">
            <button
              onClick={handleRestart}
              className="flex-1 py-3 bg-primary text-primary-fg rounded-lg hover:bg-primary/90 font-medium"
            >
              继续复习
            </button>
            <button
              onClick={handleExit}
              className="flex-1 py-3 bg-bg-secondary text-text-primary border border-border rounded-lg hover:bg-bg-tertiary font-medium"
            >
              返回
            </button>
          </div>
        </div>
      </div>
    );
  }

  const currentCard = dueCards[currentIndex];
  const progress = ((currentIndex + 1) / dueCards.length) * 100;

  return (
    <div className="h-full flex flex-col bg-bg-primary">
      {/* 顶部进度条 */}
      <div className="bg-bg-secondary border-b border-border">
        <div className="px-6 py-3 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <button
              onClick={handleExit}
              className="p-1 hover:bg-bg-tertiary rounded transition-colors"
            >
              <ArrowLeft size={20} className="text-text-secondary" />
            </button>
            <div className="flex items-center gap-2 text-sm text-text-secondary">
              <RefreshCw size={16} />
              <span>复习模式</span>
            </div>
          </div>

          <div className="flex items-center gap-4 text-sm">
            <div className="flex items-center gap-1.5">
              <Clock size={14} className="text-text-tertiary" />
              <span className="text-text-secondary">
                {Math.floor((Date.now() - session.startTime) / 60000)}分钟
              </span>
            </div>
            <div className="text-text-secondary">
              {currentIndex + 1} / {dueCards.length}
            </div>
            <div className="flex items-center gap-1.5">
              <TrendingUp size={14} className="text-green-500" />
              <span className="text-text-secondary">{session.correctRate.toFixed(0)}%</span>
            </div>
          </div>
        </div>

        {/* 进度条 */}
        <div className="h-1 bg-bg-tertiary">
          <div
            className="h-full bg-primary transition-all duration-300"
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>

      {/* 主内容区 */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-3xl mx-auto px-6 py-8">
          {/* 卡牌信息卡片 */}
          <div className="mb-4 p-3 bg-bg-secondary border border-border rounded-lg flex items-center justify-between text-xs">
            <div className="flex items-center gap-4">
              <div>
                <span className="text-text-tertiary">复习次数:</span>
                <span className="ml-1 font-semibold text-text-primary">{currentCard.reps}</span>
              </div>
              <div>
                <span className="text-text-tertiary">稳定性:</span>
                <span className="ml-1 font-semibold text-text-primary">
                  {currentCard.stability.toFixed(1)}
                </span>
              </div>
              <div>
                <span className="text-text-tertiary">阶段:</span>
                <span className="ml-1 font-semibold text-primary">{currentCard.phase}</span>
              </div>
            </div>
            {!showAnswer && wordDetail?.imageUrl && (
              <span className="text-text-tertiary">🖼️ 图片记忆</span>
            )}
          </div>

          {/* 背景图片（未显示答案时） */}
          {!showAnswer && wordDetail?.imageUrl && (
            <div className="relative mb-6 rounded-xl overflow-hidden" style={{ height: '280px' }}>
              <img
                src={wordDetail.imageUrl}
                alt={currentCard.word}
                className="w-full h-full object-cover"
                onError={(e) => {
                  (e.target as HTMLImageElement).style.display = 'none';
                }}
              />
              <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent" />
              <div className="absolute bottom-0 left-0 right-0 p-6 text-center">
                <h1 className="text-5xl font-bold text-white drop-shadow-lg mb-2">
                  {currentCard.word}
                </h1>
                {wordDetail.phonetic && (
                  <p className="text-lg text-white/90 drop-shadow">/{wordDetail.phonetic}/</p>
                )}
              </div>
            </div>
          )}

          {/* 单词标题（无图或显示答案时） */}
          {(!wordDetail?.imageUrl || showAnswer) && (
            <div className="text-center mb-8">
              <h1 className="text-6xl font-bold text-text-primary mb-3">{currentCard.word}</h1>
              {wordDetail?.phonetic && (
                <div className="flex items-center justify-center gap-3">
                  <span className="text-xl text-text-secondary">/{wordDetail.phonetic}/</span>
                  {wordDetail.usAudioUrl && (
                    <button
                      onClick={() => playAudio(wordDetail.usAudioUrl!)}
                      className="flex items-center gap-1 px-2 py-1 text-xs bg-bg-tertiary text-primary rounded hover:bg-bg-tertiary transition-colors"
                    >
                      <Volume2 size={12} /> 美音
                    </button>
                  )}
                  {wordDetail.ukAudioUrl && (
                    <button
                      onClick={() => playAudio(wordDetail.ukAudioUrl!)}
                      className="flex items-center gap-1 px-2 py-1 text-xs bg-green-50 text-green-600 rounded hover:bg-green-100 transition-colors"
                    >
                      <Volume2 size={12} /> 英音
                    </button>
                  )}
                </div>
              )}
            </div>
          )}

          {/* 显示答案按钮 */}
          {!showAnswer && (
            <div className="text-center py-12">
              <button
                onClick={() => setShowAnswer(true)}
                className="px-12 py-4 bg-primary text-primary-fg rounded-xl hover:bg-primary/90 text-lg font-medium shadow-lg hover:shadow-xl transition-all hover:scale-105"
              >
                显示答案
              </button>
              <p className="mt-4 text-sm text-text-tertiary">先尝试回忆单词的意思</p>
              <p className="mt-2 text-xs text-text-tertiary">
                按{' '}
                <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-text-secondary">空格</kbd>{' '}
                或{' '}
                <kbd className="px-1.5 py-0.5 bg-bg-tertiary rounded text-text-secondary">回车</kbd>{' '}
                显示答案
              </p>
            </div>
          )}

          {/* 答案内容 */}
          {showAnswer && wordDetail && (
            <div className="space-y-4 animate-fadeIn">
              {/* 中文释义 */}
              {wordDetail.chineseTranslation && (
                <div className="p-5 bg-gradient-to-br from-bg-tertiary to-bg-tertiary dark:from-bg-tertiary dark:to-bg-tertiary border border-border dark:border-border rounded-xl">
                  <h3 className="text-xs font-semibold text-primary dark:text-primary mb-2 flex items-center gap-1">
                    <span>🔤</span> 中文释义
                  </h3>
                  <p className="text-lg text-text-primary leading-relaxed">
                    {wordDetail.chineseTranslation}
                  </p>
                </div>
              )}

              {/* 柯林斯词典 */}
              {wordDetail.collinsEntries && wordDetail.collinsEntries.length > 0 && (
                <div className="p-5 bg-gradient-to-br from-orange-50 to-amber-50 dark:from-orange-950/30 dark:to-amber-950/30 border border-orange-200 dark:border-orange-800 rounded-xl">
                  <h3 className="text-xs font-semibold text-orange-700 dark:text-orange-400 mb-3 flex items-center gap-1">
                    <span>📖</span> 柯林斯词典
                  </h3>
                  <div className="space-y-3">
                    {wordDetail.collinsEntries.slice(0, 3).map((entry, i) => (
                      <div key={i}>
                        <div className="flex items-center gap-2 mb-1">
                          <span className="text-xs px-2 py-0.5 bg-orange-100 dark:bg-orange-900/50 text-orange-700 dark:text-orange-400 rounded font-mono">
                            {entry.pos}
                          </span>
                          {entry.posCn && (
                            <span className="text-xs text-text-tertiary">{entry.posCn}</span>
                          )}
                        </div>
                        <p className="text-sm text-text-primary mb-2">{entry.englishDef}</p>
                        {entry.examples.slice(0, 1).map((ex, j) => (
                          <div
                            key={j}
                            className="ml-3 pl-3 border-l-2 border-orange-200 dark:border-orange-800"
                          >
                            <p className="text-xs text-text-primary italic">{ex.en}</p>
                            <p className="text-xs text-text-secondary">{ex.zh}</p>
                          </div>
                        ))}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* 英文释义（如果没有柯林斯） */}
              {wordDetail.englishDefinitions &&
                wordDetail.englishDefinitions.length > 0 &&
                !wordDetail.collinsEntries.length && (
                  <div className="p-5 ui-card">
                    <h3 className="text-xs font-semibold text-primary mb-2">英文释义</h3>
                    <ul className="space-y-1">
                      {wordDetail.englishDefinitions.slice(0, 5).map((def, i) => (
                        <li key={i} className="text-sm text-text-primary">
                          • {def}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

              {/* AI内容 */}
              {wordDetail.aiContent && (
                <>
                  {wordDetail.aiContent.etymology && (
                    <div className="p-5 bg-gradient-to-br from-bg-tertiary to-bg-tertiary dark:from-bg-tertiary dark:to-bg-tertiary border border-border dark:border-border rounded-xl">
                      <h3 className="text-xs font-semibold text-primary dark:text-primary mb-2 flex items-center gap-1">
                        <span>🔤</span> 词源分析
                      </h3>
                      <p className="text-sm text-text-primary">
                        {wordDetail.aiContent.etymology.origin}
                      </p>
                    </div>
                  )}

                  {wordDetail.aiContent.mnemonics && wordDetail.aiContent.mnemonics.length > 0 && (
                    <div className="p-5 bg-gradient-to-br from-amber-50 to-yellow-50 dark:from-amber-950/30 dark:to-yellow-950/30 border border-amber-200 dark:border-amber-800 rounded-xl">
                      <h3 className="text-xs font-semibold text-amber-700 dark:text-amber-400 mb-2 flex items-center gap-1">
                        <span>💡</span> 助记法
                      </h3>
                      {wordDetail.aiContent.mnemonics.slice(0, 2).map((m, i) => (
                        <p key={i} className="text-sm text-text-primary mb-1 last:mb-0">
                          {m.content}
                        </p>
                      ))}
                    </div>
                  )}

                  {wordDetail.aiContent.examples && wordDetail.aiContent.examples.length > 0 && (
                    <div className="p-5 bg-gradient-to-br from-green-50 to-emerald-50 dark:from-green-950/30 dark:to-emerald-950/30 border border-green-200 dark:border-green-800 rounded-xl">
                      <h3 className="text-xs font-semibold text-green-700 dark:text-green-400 mb-2 flex items-center gap-1">
                        <span>📝</span> AI 例句
                      </h3>
                      {wordDetail.aiContent.examples.slice(0, 2).map((ex, i) => (
                        <div key={i} className="mb-2 last:mb-0">
                          <p className="text-sm text-text-primary italic">{ex.text}</p>
                          <p className="text-xs text-text-secondary mt-0.5">{ex.context}</p>
                        </div>
                      ))}
                    </div>
                  )}
                </>
              )}

              {/* 例句 */}
              {wordDetail.examples && wordDetail.examples.length > 0 && (
                <div className="p-5 bg-gradient-to-br from-bg-tertiary to-bg-tertiary dark:from-bg-tertiary dark:to-bg-tertiary border border-border dark:border-border rounded-xl">
                  <h3 className="text-xs font-semibold text-neutral-500 dark:text-neutral-500 mb-2 flex items-center gap-1">
                    <span>📝</span> 双语例句
                  </h3>
                  <div className="space-y-2">
                    {wordDetail.examples.slice(0, 3).map((ex, i) => (
                      <div key={i} className="pl-3 border-l-2 border-border dark:border-border">
                        <p className="text-sm text-text-primary">{ex.en}</p>
                        <p className="text-xs text-text-secondary mt-0.5">{ex.zh}</p>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* AI配置提示 */}
              {!wordDetail.aiContent && (
                <div className="p-4 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg text-center">
                  <Sparkles size={20} className="inline text-amber-500 mb-1 animate-pulse" />
                  <p className="text-xs text-text-secondary mt-1">AI 内容正在生成中...</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* 底部评分按钮 */}
      {showAnswer && (
        <div className="border-t border-border bg-bg-secondary p-6 shadow-lg">
          <div className="max-w-3xl mx-auto">
            <p className="text-center text-sm text-text-secondary mb-4">你记住这个单词了吗？</p>
            <div className="grid grid-cols-4 gap-3">
              <button
                onClick={() => void handleRate('Again')}
                disabled={submitting}
                className="py-4 bg-red-500 hover:bg-red-600 text-white rounded-xl font-medium transition-all hover:scale-[1.02] disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg"
              >
                <div className="text-2xl mb-1">😰</div>
                <div className="text-sm">忘记</div>
                <div className="text-xs opacity-75 mt-1">&lt; 1分钟</div>
                <div className="text-xs opacity-50 mt-0.5">按 1</div>
              </button>
              <button
                onClick={() => void handleRate('Hard')}
                disabled={submitting}
                className="py-4 bg-orange-500 hover:bg-orange-600 text-white rounded-xl font-medium transition-all hover:scale-[1.02] disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg"
              >
                <div className="text-2xl mb-1">😕</div>
                <div className="text-sm">困难</div>
                <div className="text-xs opacity-75 mt-1">&lt; 10分钟</div>
                <div className="text-xs opacity-50 mt-0.5">按 2</div>
              </button>
              <button
                onClick={() => void handleRate('Good')}
                disabled={submitting}
                className="py-4 bg-green-500 hover:bg-green-600 text-white rounded-xl font-medium transition-all hover:scale-[1.02] disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg"
              >
                <div className="text-2xl mb-1">😊</div>
                <div className="text-sm">良好</div>
                <div className="text-xs opacity-75 mt-1">~4天</div>
                <div className="text-xs opacity-50 mt-0.5">按 3</div>
              </button>
              <button
                onClick={() => void handleRate('Easy')}
                disabled={submitting}
                className="py-4 bg-primary hover:bg-primary-hover text-primary-fg rounded-xl font-medium transition-all hover:scale-[1.02] disabled:opacity-50 disabled:cursor-not-allowed shadow-md hover:shadow-lg"
              >
                <div className="text-2xl mb-1">🤩</div>
                <div className="text-sm">简单</div>
                <div className="text-xs opacity-75 mt-1">~15天</div>
                <div className="text-xs opacity-50 mt-0.5">按 4</div>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
