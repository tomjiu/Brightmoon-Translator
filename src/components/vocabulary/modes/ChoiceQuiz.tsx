import { useState, useEffect, useCallback } from 'react';
import { CheckCircle, XCircle, ChevronRight, RotateCcw, Keyboard } from 'lucide-react';
import { recordQuizResult } from '../../../services/vocabulary';

interface ChoiceQuestion {
  word: string;
  question: string;
  options: string[];
  correctIndex: number;
  explanation?: string;
}

interface ChoiceQuizProps {
  questions: ChoiceQuestion[];
  onComplete: (correct: number, total: number) => void;
}

export function ChoiceQuiz({ questions, onComplete }: ChoiceQuizProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [showResult, setShowResult] = useState(false);
  const [correctCount, setCorrectCount] = useState(0);
  const [finished, setFinished] = useState(false);

  const current = questions[currentIndex];

  const handleSelect = useCallback(
    (index: number) => {
      if (showResult || !current) return;
      setSelectedIndex(index);
      setShowResult(true);

      const isCorrect = index === current.correctIndex;
      if (isCorrect) {
        setCorrectCount((c) => c + 1);
      }

      // T15: 答题结果记录到学习系统（弱项统计 + QuizCompleted 事件）
      void recordQuizResult(
        current.word,
        'choice',
        isCorrect,
        current.options[index],
        current.options[current.correctIndex],
      ).catch((err: unknown) => console.error('记录答题结果失败:', err));
    },
    [showResult, current],
  );

  const handleNext = useCallback(() => {
    if (currentIndex + 1 >= questions.length) {
      setFinished(true);
      onComplete(correctCount, questions.length);
    } else {
      setCurrentIndex((i) => i + 1);
      setSelectedIndex(null);
      setShowResult(false);
    }
  }, [currentIndex, questions.length, correctCount, onComplete]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (finished) return;

      if (showResult) {
        if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault();
          handleNext();
        }
        return;
      }

      // Select option with 1-4 or A-D
      const optionKeys = ['1', '2', '3', '4'];
      const alphaKeys = ['a', 'b', 'c', 'd'];
      const key = e.key.toLowerCase();

      if (optionKeys.includes(key)) {
        handleSelect(parseInt(key) - 1);
      } else if (alphaKeys.includes(key)) {
        handleSelect(alphaKeys.indexOf(key));
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showResult, finished, handleSelect, handleNext]);

  if (finished) {
    const rate = Math.round((correctCount / questions.length) * 100);
    return (
      <div className="flex flex-col items-center justify-center h-full space-y-6">
        <div className="text-6xl">{rate >= 80 ? '🎉' : rate >= 60 ? '👍' : '💪'}</div>
        <div className="text-3xl font-bold">{rate}%</div>
        <div className="text-gray-400">
          答对 {correctCount} / {questions.length} 题
        </div>
        <div className="flex gap-4 mt-6">
          <button
            onClick={() => window.location.reload()}
            className="flex items-center gap-2 px-6 py-3 bg-gray-700 hover:bg-gray-600 rounded-lg transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            再来一轮
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-2xl mx-auto p-6 space-y-6">
      {/* Progress */}
      <div className="flex items-center gap-4">
        <div className="text-sm text-gray-400">
          {currentIndex + 1} / {questions.length}
        </div>
        <div className="flex-1 h-2 bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-primary transition-all duration-300"
            style={{ width: `${((currentIndex + 1) / questions.length) * 100}%` }}
          />
        </div>
        <div className="text-sm text-green-400">✓ {correctCount}</div>
      </div>

      {/* Question */}
      <div className="bg-gray-800 rounded-lg p-8">
        <h3 className="text-lg text-gray-300 mb-2">选择题</h3>
        <p className="text-2xl font-bold">{current.question}</p>
      </div>

      {/* Options */}
      <div className="space-y-3">
        {current.options.map((option, idx) => {
          let bgClass = 'bg-gray-800 hover:bg-gray-700';
          if (showResult) {
            if (idx === current.correctIndex) {
              bgClass = 'bg-green-500/20 border-green-500';
            } else if (idx === selectedIndex && idx !== current.correctIndex) {
              bgClass = 'bg-red-500/20 border-red-500';
            }
          } else if (selectedIndex === idx) {
            bgClass = 'bg-white/10 border-border';
          }

          return (
            <button
              key={idx}
              onClick={() => handleSelect(idx)}
              disabled={showResult}
              className={`w-full text-left px-6 py-4 rounded-lg border border-transparent transition-all ${bgClass}`}
            >
              <div className="flex items-center gap-3">
                <span className="w-8 h-8 flex items-center justify-center rounded-full bg-gray-700 text-sm font-mono">
                  {String.fromCharCode(65 + idx)}
                </span>
                <span className="text-lg">{option}</span>
                {showResult && idx === current.correctIndex && (
                  <CheckCircle className="w-5 h-5 text-green-400 ml-auto" />
                )}
                {showResult && idx === selectedIndex && idx !== current.correctIndex && (
                  <XCircle className="w-5 h-5 text-red-400 ml-auto" />
                )}
              </div>
            </button>
          );
        })}
      </div>

      {/* Next Button */}
      {showResult && (
        <button
          onClick={handleNext}
          className="w-full flex items-center justify-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover rounded-lg transition-colors"
        >
          {currentIndex + 1 >= questions.length ? '查看结果' : '下一题'}
          <ChevronRight className="w-4 h-4" />
        </button>
      )}

      {/* Keyboard hints */}
      {!showResult && (
        <div className="flex items-center justify-center gap-3 text-xs text-gray-500">
          <Keyboard className="w-3 h-3" />
          <span>按 1-4 或 A-D 选择</span>
        </div>
      )}
      {showResult && (
        <div className="flex items-center justify-center gap-3 text-xs text-gray-500">
          <Keyboard className="w-3 h-3" />
          <span>按空格或回车继续</span>
        </div>
      )}
    </div>
  );
}
