import { useState, useRef } from 'react';
import { CheckCircle, XCircle, ChevronRight, RotateCcw, Keyboard } from 'lucide-react';
import { recordQuizResult } from '../../../services/vocabulary';

interface SpellingQuestion {
  definition: string;
  hint: string;
  answer: string;
  example?: string;
}

interface SpellingQuizProps {
  questions: SpellingQuestion[];
  onComplete: (correct: number, total: number) => void;
}

export function SpellingQuiz({ questions, onComplete }: SpellingQuizProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [input, setInput] = useState('');
  const [showResult, setShowResult] = useState(false);
  const [correctCount, setCorrectCount] = useState(0);
  const [finished, setFinished] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const current = questions[currentIndex];
  if (!current) return null;

  const handleCheck = () => {
    if (!input.trim()) return;
    setShowResult(true);

    const isCorrect = input.trim().toLowerCase() === current.answer.toLowerCase();
    if (isCorrect) {
      setCorrectCount((c) => c + 1);
    }

    // T15: 答题结果记录到学习系统（弱项统计 + QuizCompleted 事件）
    void recordQuizResult(current.answer, 'spelling', isCorrect, input.trim(), current.answer).catch(
      (err: unknown) => console.error('记录答题结果失败:', err),
    );
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      if (showResult) {
        handleNext();
      } else {
        handleCheck();
      }
    }
  };

  const handleNext = () => {
    if (currentIndex + 1 >= questions.length) {
      setFinished(true);
      onComplete(correctCount, questions.length);
    } else {
      setCurrentIndex((i) => i + 1);
      setInput('');
      setShowResult(false);
      inputRef.current?.focus();
    }
  };

  if (finished) {
    const rate = Math.round((correctCount / questions.length) * 100);
    return (
      <div className="flex flex-col items-center justify-center h-full space-y-6">
        <div className="text-6xl">{rate >= 80 ? '🎉' : rate >= 60 ? '👍' : '💪'}</div>
        <div className="text-3xl font-bold">{rate}%</div>
        <div className="text-gray-400">
          拼写正确 {correctCount} / {questions.length} 题
        </div>
        <button
          onClick={() => window.location.reload()}
          className="flex items-center gap-2 px-6 py-3 bg-gray-700 hover:bg-gray-600 rounded-lg transition-colors"
        >
          <RotateCcw className="w-4 h-4" />
          再来一轮
        </button>
      </div>
    );
  }

  const isCorrect = input.trim().toLowerCase() === current.answer.toLowerCase();

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
        <h3 className="text-lg text-gray-300 mb-2">拼写题</h3>
        <p className="text-2xl font-bold mb-4">{current.definition}</p>
        <p className="text-sm text-gray-400">提示：{current.hint}</p>
      </div>

      {/* Input */}
      <div className="relative">
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={showResult}
          placeholder="输入单词拼写..."
          autoFocus
          className={`w-full px-6 py-4 text-xl bg-gray-800 rounded-lg focus:outline-none focus:ring-2 ${
            showResult
              ? isCorrect
                ? 'ring-2 ring-green-500'
                : 'ring-2 ring-red-500'
              : 'focus:ring-primary/40'
          }`}
        />
        {showResult && (
          <div className="absolute right-4 top-1/2 -translate-y-1/2">
            {isCorrect ? (
              <CheckCircle className="w-6 h-6 text-green-400" />
            ) : (
              <XCircle className="w-6 h-6 text-red-400" />
            )}
          </div>
        )}
      </div>

      {/* Correct Answer */}
      {showResult && !isCorrect && (
        <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-4">
          <p className="text-sm text-gray-400">正确答案：</p>
          <p className="text-xl font-bold text-red-400">{current.answer}</p>
        </div>
      )}

      {/* Action Buttons */}
      <div className="flex gap-3">
        {!showResult ? (
          <button
            onClick={handleCheck}
            disabled={!input.trim()}
            className="flex-1 px-6 py-3 bg-primary hover:bg-primary-hover disabled:bg-gray-700 disabled:text-gray-500 rounded-lg transition-colors"
          >
            检查
          </button>
        ) : (
          <button
            onClick={handleNext}
            className="flex-1 flex items-center justify-center gap-2 px-6 py-3 bg-primary hover:bg-primary-hover rounded-lg transition-colors"
          >
            {currentIndex + 1 >= questions.length ? '查看结果' : '下一题'}
            <ChevronRight className="w-4 h-4" />
          </button>
        )}
      </div>

      {/* Keyboard hints */}
      <div className="flex items-center justify-center gap-3 text-xs text-gray-500">
        <Keyboard className="w-3 h-3" />
        <span>输入后按 Enter 检查，再按 Enter 继续</span>
      </div>
    </div>
  );
}
