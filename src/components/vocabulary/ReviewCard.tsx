// ReviewCard - 复习卡牌组件

import { useState } from 'react';
import { useCard, useSubmitReview } from '../../hooks/useVocabulary';
import { useVocabularyStore } from '../../stores/vocabularyStore';
import { Rating, getRatingDisplayText, getRatingColorClass } from '../../services/vocabulary';

interface ReviewCardProps {
  cardId: string;
  onComplete?: () => void;
  className?: string;
}

export function ReviewCard({ cardId, onComplete, className = '' }: ReviewCardProps) {
  const [showAnswer, setShowAnswer] = useState(false);
  const { data: card, isLoading } = useCard(cardId);
  const { mutate: submitReview, isPending } = useSubmitReview();
  const incrementReviewed = useVocabularyStore((state) => state.incrementReviewed);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-gray-500">加载中...</div>
      </div>
    );
  }

  if (!card) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500">
        卡牌不存在
      </div>
    );
  }

  const handleRate = (rating: Rating) => {
    submitReview(
      { cardId, rating },
      {
        onSuccess: () => {
          incrementReviewed(rating !== Rating.Again);
          setShowAnswer(false);
          onComplete?.();
        },
      },
    );
  };

  return (
    <div className={`flex flex-col h-full ${className}`}>
      {/* 卡牌内容 */}
      <div className="flex-1 flex flex-col items-center justify-center p-8">
        {/* 问题面 */}
        <div className="w-full max-w-2xl">
          <div className="text-center mb-8">
            <h1 className="text-5xl font-bold mb-4">{card.word}</h1>
            {card.base_data.phonetic && (
              <p className="text-xl text-gray-600 mb-2">/{card.base_data.phonetic}/</p>
            )}
            {!showAnswer && (
              <button
                onClick={() => setShowAnswer(true)}
                className="mt-6 px-8 py-3 bg-blue-500 text-white rounded-lg hover:bg-blue-600 text-lg"
              >
                显示答案
              </button>
            )}
          </div>

          {/* 答案面 */}
          {showAnswer && (
            <div className="space-y-6 animate-fadeIn">
              {/* 释义 */}
              {card.base_data.definitions.length > 0 && (
                <div className="bg-white p-6 rounded-lg shadow-sm border">
                  <h3 className="text-sm font-semibold text-gray-700 mb-2">释义</h3>
                  <ul className="space-y-1">
                    {card.base_data.definitions.map((def, i) => (
                      <li key={i} className="text-gray-700">
                        • {def}
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {/* 翻译 */}
              {card.base_data.translation && (
                <div className="bg-white p-6 rounded-lg shadow-sm border">
                  <h3 className="text-sm font-semibold text-gray-700 mb-2">中文</h3>
                  <p className="text-gray-700">{card.base_data.translation}</p>
                </div>
              )}

              {/* 助记法 */}
              {card.ai_content?.mnemonics && card.ai_content.mnemonics.length > 0 && (
                <div className="bg-yellow-50 p-6 rounded-lg border border-yellow-200">
                  <h3 className="text-sm font-semibold text-gray-700 mb-2">助记法</h3>
                  <p className="text-gray-700">{card.ai_content.mnemonics[0].content}</p>
                </div>
              )}

              {/* 例句 */}
              {card.ai_content?.examples && card.ai_content.examples.length > 0 && (
                <div className="bg-green-50 p-6 rounded-lg border border-green-200">
                  <h3 className="text-sm font-semibold text-gray-700 mb-2">例句</h3>
                  <p className="text-gray-700 italic">{card.ai_content.examples[0].text}</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* 评分按钮 */}
      {showAnswer && (
        <div className="border-t bg-white p-6">
          <div className="max-w-2xl mx-auto">
            <p className="text-center text-sm text-gray-600 mb-4">你记住这个单词了吗？</p>
            <div className="grid grid-cols-4 gap-4">
              {[Rating.Again, Rating.Hard, Rating.Good, Rating.Easy].map((rating) => (
                <button
                  key={rating}
                  onClick={() => handleRate(rating)}
                  disabled={isPending}
                  className={`py-4 rounded-lg font-medium transition-all hover:scale-105 disabled:opacity-50 disabled:cursor-not-allowed ${getRatingColorClass(rating)} ${getRatingButtonClass(rating)}`}
                >
                  {getRatingDisplayText(rating)}
                </button>
              ))}
            </div>
            <div className="mt-4 grid grid-cols-4 gap-4 text-xs text-center text-gray-500">
              <div>&lt; 1分钟</div>
              <div>&lt; 10分钟</div>
              <div>{Math.round(card.fsrs_state.stability)}天</div>
              <div>{Math.round(card.fsrs_state.stability * 1.5)}天</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function getRatingButtonClass(rating: Rating): string {
  switch (rating) {
    case Rating.Again:
      return 'bg-red-100 hover:bg-red-200 border-2 border-red-300';
    case Rating.Hard:
      return 'bg-orange-100 hover:bg-orange-200 border-2 border-orange-300';
    case Rating.Good:
      return 'bg-green-100 hover:bg-green-200 border-2 border-green-300';
    case Rating.Easy:
      return 'bg-blue-100 hover:bg-blue-200 border-2 border-blue-300';
  }
}
