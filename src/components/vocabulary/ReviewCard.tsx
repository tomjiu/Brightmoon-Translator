// ReviewCard - 复习卡牌组件

import { useState } from 'react';
import { useCard, useSubmitReview } from '../../hooks/useVocabulary';
import { useVocabularyStore } from '../../stores/vocabularyStore';
import { Rating } from '../../services/vocabulary';

interface ReviewCardProps {
  cardId: string;
  onComplete?: () => void;
}

const RATING_CONFIG = {
  [Rating.Again]: { label: '重来', color: 'bg-red-500 hover:bg-red-600', hint: '< 1分钟' },
  [Rating.Hard]: { label: '困难', color: 'bg-orange-500 hover:bg-orange-600', hint: '< 10分钟' },
  [Rating.Good]: { label: '良好', color: 'bg-green-500 hover:bg-green-600', hint: '1天' },
  [Rating.Easy]: { label: '简单', color: 'bg-primary hover:bg-primary-hover', hint: '4天' },
};

export function ReviewCard({ cardId, onComplete }: ReviewCardProps) {
  const [showAnswer, setShowAnswer] = useState(false);
  const { data: card, isLoading } = useCard(cardId);
  const { mutate: submitReview, isPending } = useSubmitReview();
  const incrementReviewed = useVocabularyStore((state) => state.incrementReviewed);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-text-secondary">加载中...</div>
      </div>
    );
  }

  if (!card) {
    return (
      <div className="flex items-center justify-center h-full text-text-tertiary">卡牌不存在</div>
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
    <div className="flex flex-col h-full">
      {/* 卡牌内容 */}
      <div className="flex-1 flex flex-col items-center justify-center p-8">
        <div className="w-full max-w-2xl">
          {/* 问题面 */}
          <div className="text-center mb-8">
            <h1 className="text-5xl font-bold text-text-primary mb-3">{card.word}</h1>
            {card.base_data.phonetic && (
              <p className="text-lg text-text-secondary mb-4">/{card.base_data.phonetic}/</p>
            )}
            {!showAnswer && (
              <button
                onClick={() => setShowAnswer(true)}
                className="mt-6 px-8 py-3 bg-primary text-primary-fg rounded-lg hover:bg-primary/90 text-lg transition-colors"
              >
                显示答案
              </button>
            )}
          </div>

          {/* 答案面 */}
          {showAnswer && (
            <div className="space-y-4 animate-fadeIn">
              {card.base_data.translation && (
                <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                  <h3 className="text-xs font-semibold text-primary mb-1.5">中文释义</h3>
                  <p className="text-text-primary">{card.base_data.translation}</p>
                </div>
              )}

              {card.base_data.definitions.length > 0 && (
                <div className="p-4 bg-bg-secondary border border-border rounded-lg">
                  <h3 className="text-xs font-semibold text-primary mb-1.5">英文释义</h3>
                  <ul className="space-y-0.5">
                    {card.base_data.definitions.map((def, i) => (
                      <li key={i} className="text-sm text-text-primary">
                        • {def}
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {card.ai_content?.mnemonics && card.ai_content.mnemonics.length > 0 && (
                <div className="p-4 bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-800 rounded-lg">
                  <h3 className="text-xs font-semibold text-amber-700 dark:text-amber-400 mb-1.5">
                    💡 助记法
                  </h3>
                  <p className="text-sm text-text-primary">
                    {card.ai_content.mnemonics[0].content}
                  </p>
                </div>
              )}

              {card.ai_content?.examples && card.ai_content.examples.length > 0 && (
                <div className="p-4 bg-green-50 dark:bg-green-950/20 border border-green-200 dark:border-green-800 rounded-lg">
                  <h3 className="text-xs font-semibold text-green-700 dark:text-green-400 mb-1.5">
                    📝 例句
                  </h3>
                  <p className="text-sm text-text-primary italic">
                    {card.ai_content.examples[0].text}
                  </p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* 评分按钮 */}
      {showAnswer && (
        <div className="border-t border-border bg-bg-secondary p-6">
          <div className="max-w-2xl mx-auto">
            <p className="text-center text-sm text-text-secondary mb-3">你记住这个单词了吗？</p>
            <div className="grid grid-cols-4 gap-3">
              {([Rating.Again, Rating.Hard, Rating.Good, Rating.Easy] as const).map((rating) => {
                const cfg = RATING_CONFIG[rating];
                return (
                  <button
                    key={rating}
                    onClick={() => handleRate(rating)}
                    disabled={isPending}
                    className={`py-3 rounded-lg text-white font-medium transition-all hover:scale-[1.02] disabled:opacity-50 ${cfg.color}`}
                  >
                    {cfg.label}
                  </button>
                );
              })}
            </div>
            <div className="mt-2 grid grid-cols-4 gap-3 text-xs text-center text-text-tertiary">
              <div>{RATING_CONFIG[Rating.Again].hint}</div>
              <div>{RATING_CONFIG[Rating.Hard].hint}</div>
              <div>{RATING_CONFIG[Rating.Good].hint}</div>
              <div>{RATING_CONFIG[Rating.Easy].hint}</div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
