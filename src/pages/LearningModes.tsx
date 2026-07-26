import { useState } from 'react';
import { Brain, Keyboard, FileText, Layers, Loader2 } from 'lucide-react';
import { ChoiceQuiz } from '../components/vocabulary/modes/ChoiceQuiz';
import { SpellingQuiz } from '../components/vocabulary/modes/SpellingQuiz';
import { ClozeQuiz } from '../components/vocabulary/modes/ClozeQuiz';
import { SwipeReview } from '../components/vocabulary/modes/SwipeReview';
import {
  generateChoiceQuestions,
  generateSpellingQuestions,
  generateClozeQuestions,
  getSwipeCards,
  submitSwipeRating,
  type ChoiceQuestion,
  type SpellingQuestion,
  type ClozeQuestion,
} from '../services/learningMode';
import type { CardInfo } from '../services/vocabulary';
import { useToastStore } from '../stores/toastStore';

type LearningMode = 'menu' | 'choice' | 'spelling' | 'cloze' | 'swipe';

const MODES = [
  {
    id: 'choice' as const,
    icon: <Brain className="w-8 h-8" />,
    title: '选择题',
    description: '4选1，测试词汇理解',
    color: 'bg-white/10 text-primary',
    gradient: 'from-neutral-500/20 to-neutral-600/5',
  },
  {
    id: 'spelling' as const,
    icon: <Keyboard className="w-8 h-8" />,
    title: '拼写题',
    description: '根据释义拼写单词',
    color: 'bg-white/10 text-primary',
    gradient: 'from-neutral-500/20 to-neutral-600/5',
  },
  {
    id: 'cloze' as const,
    icon: <FileText className="w-8 h-8" />,
    title: '填空题',
    description: '根据上下文选词填空',
    color: 'bg-yellow-500/20 text-yellow-400',
    gradient: 'from-yellow-500/20 to-yellow-600/5',
  },
  {
    id: 'swipe' as const,
    icon: <Layers className="w-8 h-8" />,
    title: '快速复习',
    description: '卡片翻转，左右评分',
    color: 'bg-white/10 text-primary',
    gradient: 'from-white/10 to-neutral-600/5',
  },
];

const QUIZ_COUNT = 10;

export default function LearningModes() {
  const addToast = useToastStore((s) => s.addToast);
  const [mode, setMode] = useState<LearningMode>('menu');
  const [loading, setLoading] = useState(false);
  const [choiceQuestions, setChoiceQuestions] = useState<ChoiceQuestion[]>([]);
  const [spellingQuestions, setSpellingQuestions] = useState<SpellingQuestion[]>([]);
  const [clozeQuestions, setClozeQuestions] = useState<ClozeQuestion[]>([]);
  const [swipeCards, setSwipeCards] = useState<CardInfo[]>([]);

  const handleStartMode = async (selectedMode: LearningMode) => {
    setLoading(true);
    try {
      switch (selectedMode) {
        case 'choice': {
          const qs = await generateChoiceQuestions(null, QUIZ_COUNT);
          setChoiceQuestions(qs);
          break;
        }
        case 'spelling': {
          const qs = await generateSpellingQuestions(null, QUIZ_COUNT);
          setSpellingQuestions(qs);
          break;
        }
        case 'cloze': {
          const qs = await generateClozeQuestions(null, QUIZ_COUNT);
          setClozeQuestions(qs);
          break;
        }
        case 'swipe': {
          const cards = await getSwipeCards(QUIZ_COUNT);
          setSwipeCards(cards);
          break;
        }
      }
      setMode(selectedMode);
    } catch (error) {
      console.error('加载题目失败:', error);
      addToast({
        type: 'error',
        message: '加载题目失败，请确保已有学习数据（先创建学习计划并学习一些单词）',
        duration: 4000,
      });
    } finally {
      setLoading(false);
    }
  };

  const handleComplete = (_correct: number, _total: number) => {
    // 结果由子组件展示
  };

  const handleSwipeRate = async (cardId: string, rating: 'Again' | 'Hard' | 'Good' | 'Easy') => {
    try {
      await submitSwipeRating(cardId, rating);
    } catch (error) {
      console.error('提交评分失败:', error);
    }
  };

  if (mode !== 'menu') {
    return (
      <div className="h-full flex flex-col">
        {/* Top bar */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-border bg-bg-secondary">
          <button
            onClick={() => setMode('menu')}
            className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary hover:bg-bg-tertiary rounded-lg transition-colors"
          >
            ← 返回选择
          </button>
          <span className="text-sm text-text-secondary">
            {MODES.find((m) => m.id === mode)?.title}
          </span>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {mode === 'choice' && (
            <ChoiceQuiz questions={choiceQuestions} onComplete={handleComplete} />
          )}
          {mode === 'spelling' && (
            <SpellingQuiz questions={spellingQuestions} onComplete={handleComplete} />
          )}
          {mode === 'cloze' && <ClozeQuiz questions={clozeQuestions} onComplete={handleComplete} />}
          {mode === 'swipe' && (
            <SwipeReview
              cards={swipeCards}
              onRate={handleSwipeRate}
              onComplete={() => setMode('menu')}
            />
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto p-8">
      <div className="max-w-3xl mx-auto">
        <div className="mb-8">
          <h1 className="text-3xl font-bold mb-2">学习模式</h1>
          <p className="text-text-secondary">选择不同的练习方式，让词汇学习更有趣、更高效</p>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-20">
            <Loader2 className="w-8 h-8 animate-spin text-primary" />
            <span className="ml-3 text-text-secondary">正在生成题目...</span>
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-4">
            {MODES.map((m) => (
              <button
                key={m.id}
                onClick={() => handleStartMode(m.id)}
                className={`bg-gradient-to-br ${m.gradient} border border-border rounded-xl p-6 text-left hover:border-primary/50 hover:scale-[1.02] transition-all group`}
              >
                <div className={`inline-flex p-3 rounded-lg mb-4 ${m.color}`}>{m.icon}</div>
                <h3 className="text-xl font-bold mb-2 group-hover:text-primary transition-colors">
                  {m.title}
                </h3>
                <p className="text-sm text-text-secondary">{m.description}</p>
                <div className="mt-4 text-xs text-text-tertiary">{QUIZ_COUNT} 题/轮</div>
              </button>
            ))}
          </div>
        )}

        {/* Tips */}
        <div className="mt-8 p-4 bg-bg-secondary rounded-lg border border-border">
          <h3 className="text-sm font-semibold text-text-primary mb-2">💡 学习建议</h3>
          <ul className="text-xs text-text-secondary space-y-1">
            <li>• 先创建学习计划并学习一些单词，题目会自动生成</li>
            <li>• 选择题适合测试词汇理解能力</li>
            <li>• 拼写题能强化单词拼写记忆</li>
            <li>• 填空题帮助理解单词在语境中的用法</li>
            <li>• 快速复习模式适合批量复习已有单词</li>
          </ul>
        </div>
      </div>
    </div>
  );
}
