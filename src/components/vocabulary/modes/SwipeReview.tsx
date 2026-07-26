import { useState, useEffect, useCallback } from 'react';
import { RotateCcw, Keyboard } from 'lucide-react';

interface SwipeCard {
  id: string;
  word: string;
}

interface SwipeReviewProps {
  cards: SwipeCard[];
  onRate: (cardId: string, rating: 'Again' | 'Hard' | 'Good' | 'Easy') => void;
  onComplete: (results: Record<string, string>) => void;
}

export function SwipeReview({ cards, onRate, onComplete }: SwipeReviewProps) {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [flipped, setFlipped] = useState(false);
  const [results, setResults] = useState<Record<string, string>>({});
  const [exitDirection, setExitDirection] = useState<'left' | 'right' | null>(null);

  const current = cards[currentIndex];

  const handleRate = useCallback(
    (rating: 'Again' | 'Hard' | 'Good' | 'Easy') => {
      if (!current) return;
      const direction = rating === 'Again' || rating === 'Hard' ? 'left' : 'right';
      setExitDirection(direction);
      onRate(current.id, rating);
      setResults((prev) => ({ ...prev, [current.id]: rating }));

      setTimeout(() => {
        setExitDirection(null);
        setFlipped(false);
        if (currentIndex + 1 >= cards.length) {
          onComplete({ ...results, [current.id]: rating });
        } else {
          setCurrentIndex((i) => i + 1);
        }
      }, 200);
    },
    [current, currentIndex, cards.length, onRate, onComplete, results],
  );

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!flipped) {
        if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault();
          setFlipped(true);
        }
        return;
      }

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
  }, [flipped, handleRate]);

  if (!current) return null;

  const totalReviewed = Object.keys(results).length;
  const goodEasy = Object.values(results).filter((r) => r === 'Good' || r === 'Easy').length;
  const accuracy = totalReviewed > 0 ? Math.round((goodEasy / totalReviewed) * 100) : 0;

  return (
    <div className="max-w-lg mx-auto p-6 space-y-6">
      {/* Progress */}
      <div className="flex items-center gap-4">
        <div className="text-sm text-gray-400">
          {currentIndex + 1} / {cards.length}
        </div>
        <div className="flex-1 h-2 bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-primary transition-all duration-300"
            style={{ width: `${((currentIndex + 1) / cards.length) * 100}%` }}
          />
        </div>
      </div>

      {/* Card */}
      <div
        onClick={() => setFlipped(!flipped)}
        className={`bg-gray-800 rounded-2xl p-12 cursor-pointer select-none transition-all duration-200 min-h-[280px] flex items-center justify-center ${
          exitDirection === 'left'
            ? '-translate-x-full opacity-0'
            : exitDirection === 'right'
              ? 'translate-x-full opacity-0'
              : ''
        }`}
      >
        {!flipped ? (
          <div className="text-center">
            <p className="text-4xl font-bold mb-4">{current.word}</p>
            <p className="text-sm text-gray-400">点击翻转查看释义</p>
          </div>
        ) : (
          <div className="text-center">
            <p className="text-2xl font-bold text-primary mb-4">{current.word}</p>
            <p className="text-gray-300">记住这个单词了吗？</p>
            <p className="text-sm text-gray-400 mt-2">选择你的记忆程度</p>
          </div>
        )}
      </div>

      {/* Rating Buttons */}
      {flipped && (
        <div className="grid grid-cols-4 gap-3">
          <button
            onClick={() => handleRate('Again')}
            className="px-4 py-3 bg-red-500/20 hover:bg-red-500/30 text-red-400 rounded-lg transition-colors text-center"
          >
            <div className="text-lg font-bold">忘了</div>
            <div className="text-xs opacity-70">Again</div>
          </button>
          <button
            onClick={() => handleRate('Hard')}
            className="px-4 py-3 bg-yellow-500/20 hover:bg-yellow-500/30 text-yellow-400 rounded-lg transition-colors text-center"
          >
            <div className="text-lg font-bold">模糊</div>
            <div className="text-xs opacity-70">Hard</div>
          </button>
          <button
            onClick={() => handleRate('Good')}
            className="px-4 py-3 bg-green-500/20 hover:bg-green-500/30 text-green-400 rounded-lg transition-colors text-center"
          >
            <div className="text-lg font-bold">记得</div>
            <div className="text-xs opacity-70">Good</div>
          </button>
          <button
            onClick={() => handleRate('Easy')}
            className="px-4 py-3 bg-white/10 hover:bg-primary/30 text-primary rounded-lg transition-colors text-center"
          >
            <div className="text-lg font-bold">秒答</div>
            <div className="text-xs opacity-70">Easy</div>
          </button>
        </div>
      )}

      {/* Flip hint */}
      {!flipped && (
        <div className="flex justify-center gap-4">
          <button
            onClick={() => setFlipped(true)}
            className="flex items-center gap-2 px-6 py-3 bg-gray-700 hover:bg-gray-600 rounded-lg transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
            翻转
          </button>
        </div>
      )}

      {/* Stats */}
      <div className="flex items-center justify-center gap-6 text-sm">
        <div className="text-gray-400">
          已复习 <span className="font-bold text-white">{totalReviewed}</span> 张
        </div>
        {totalReviewed > 0 && (
          <div className="text-gray-400">
            正确率{' '}
            <span className={`font-bold ${accuracy >= 70 ? 'text-green-400' : 'text-yellow-400'}`}>
              {accuracy}%
            </span>
          </div>
        )}
      </div>

      {/* Keyboard hints */}
      <div className="flex items-center justify-center gap-4 text-xs text-gray-500">
        <Keyboard className="w-3 h-3" />
        <span>空格翻转</span>
        <span>1忘了 2模糊 3记得 4秒答</span>
        <span>←忘了 →秒答</span>
      </div>
    </div>
  );
}
