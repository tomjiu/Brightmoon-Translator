import { useState, useEffect, useCallback } from 'react';
import { CheckCircle, XCircle, ChevronRight, RotateCcw, Keyboard } from 'lucide-react';

interface ClozeQuestion {
  sentence: string;
  answer: string;
  options: string[];
  context?: string;
}

interface ClozeQuizProps {
  questions: ClozeQuestion[];
  onComplete: (correct: number, total: number) => void;
}

export function ClozeQuiz({ questions, onComplete }: ClozeQuizProps) {
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

      if (current.options[index] === current.answer) {
        setCorrectCount((c) => c + 1);
      }
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

      const keys = ['1', '2', '3', '4'];
      if (keys.includes(e.key)) {
        handleSelect(parseInt(e.key) - 1);
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
          填空正确 {correctCount} / {questions.length} 题
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

  return (
    <div className="max-w-2xl mx-auto p-6 space-y-6">
      {/* Progress */}
      <div className="flex items-center gap-4">
        <div className="text-sm text-gray-400">
          {currentIndex + 1} / {questions.length}
        </div>
        <div className="flex-1 h-2 bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-yellow-500 transition-all duration-300"
            style={{ width: `${((currentIndex + 1) / questions.length) * 100}%` }}
          />
        </div>
        <div className="text-sm text-green-400">✓ {correctCount}</div>
      </div>

      {/* Sentence with blank */}
      <div className="bg-gray-800 rounded-lg p-8">
        <h3 className="text-lg text-gray-300 mb-4">填空题</h3>
        <p className="text-xl leading-relaxed">
          {current.sentence.split('____').map((part, idx, arr) => (
            <span key={idx}>
              {part}
              {idx < arr.length - 1 && (
                <span
                  className={`inline-block min-w-[100px] mx-1 px-3 py-1 border-b-2 text-center font-bold ${
                    showResult
                      ? selectedIndex !== null && current.options[selectedIndex] === current.answer
                        ? 'border-green-500 text-green-400'
                        : 'border-red-500 text-red-400'
                      : 'border-border text-primary'
                  }`}
                >
                  {showResult && selectedIndex !== null ? current.options[selectedIndex] : '?'}
                </span>
              )}
            </span>
          ))}
        </p>
        {current.context && (
          <p className="text-sm text-gray-400 mt-4">💡 提示：{current.context}</p>
        )}
      </div>

      {/* Options */}
      <div className="grid grid-cols-2 gap-3">
        {current.options.map((option, idx) => {
          let bgClass = 'bg-gray-800 hover:bg-gray-700';
          if (showResult) {
            if (option === current.answer) {
              bgClass = 'bg-green-500/20 border-green-500';
            } else if (idx === selectedIndex && option !== current.answer) {
              bgClass = 'bg-red-500/20 border-red-500';
            }
          }

          return (
            <button
              key={idx}
              onClick={() => handleSelect(idx)}
              disabled={showResult}
              className={`px-6 py-4 rounded-lg border border-transparent text-left transition-all ${bgClass}`}
            >
              <div className="flex items-center justify-between">
                <span className="text-lg">{option}</span>
                {showResult && option === current.answer && (
                  <CheckCircle className="w-5 h-5 text-green-400" />
                )}
                {showResult && idx === selectedIndex && option !== current.answer && (
                  <XCircle className="w-5 h-5 text-red-400" />
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
      <div className="flex items-center justify-center gap-3 text-xs text-gray-500">
        <Keyboard className="w-3 h-3" />
        <span>按 1-4 选择选项，空格/回车继续</span>
      </div>
    </div>
  );
}
