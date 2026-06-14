// VocabularyLearning - AI 驱动的词汇学习主页面

import { useState } from 'react';
import {
  CoreVocabularyList,
  CardDetail,
  ReviewCard,
  LearningStatsPanel,
} from '../components/vocabulary';
import { useVocabularyStore } from '../stores/vocabularyStore';
import { useDueCards, useCreateCard } from '../hooks/useVocabulary';
import { BookOpen, Brain, BarChart3 } from 'lucide-react';

type ViewMode = 'browse' | 'review' | 'stats';

function VocabularyLearning() {
  const [viewMode, setViewMode] = useState<ViewMode>('browse');
  const [selectedCardId, setSelectedCardId] = useState<string | null>(null);
  const [reviewIndex, setReviewIndex] = useState(0);

  const { data: dueCards = [] } = useDueCards();
  const { mutate: createCard } = useCreateCard();
  const { startSession, endSession } = useVocabularyStore();

  const handleSelectWord = async (word: string) => {
    createCard(word, {
      onSuccess: (cardId) => {
        setSelectedCardId(cardId);
      },
    });
  };

  const handleStartReview = () => {
    if (dueCards.length > 0) {
      setReviewIndex(0);
      setViewMode('review');
      startSession();
    }
  };

  const handleReviewComplete = () => {
    const nextIndex = reviewIndex + 1;
    if (nextIndex < dueCards.length) {
      setReviewIndex(nextIndex);
    } else {
      endSession();
      setViewMode('browse');
      setReviewIndex(0);
    }
  };

  const currentReviewCard = dueCards[reviewIndex];

  return (
    <div className="h-full flex">
      {/* Left Sidebar - Navigation */}
      <div className="w-48 border-r border-border bg-bg-secondary flex flex-col">
        <div className="p-4">
          <h2 className="text-sm font-semibold text-text-primary mb-4">AI Learning System</h2>
          <div className="space-y-1">
            <button
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors ${
                viewMode === 'browse'
                  ? 'bg-primary text-white'
                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
              }`}
              onClick={() => setViewMode('browse')}
            >
              <BookOpen size={16} />
              Browse Vocabulary
            </button>
            <button
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors ${
                viewMode === 'review'
                  ? 'bg-primary text-white'
                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
              }`}
              onClick={handleStartReview}
              disabled={dueCards.length === 0}
            >
              <Brain size={16} />
              Review ({dueCards.length})
            </button>
            <button
              className={`w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors ${
                viewMode === 'stats'
                  ? 'bg-primary text-white'
                  : 'text-text-secondary hover:bg-bg-tertiary hover:text-text-primary'
              }`}
              onClick={() => setViewMode('stats')}
            >
              <BarChart3 size={16} />
              Statistics
            </button>
          </div>
        </div>

        {/* Stats Panel in Sidebar */}
        {viewMode !== 'stats' && (
          <div className="px-4 pb-4">
            <LearningStatsPanel />
          </div>
        )}
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex overflow-hidden">
        {viewMode === 'browse' && (
          <>
            {/* Vocabulary List */}
            <div className="w-96 border-r border-border">
              <CoreVocabularyList onSelectWord={handleSelectWord} />
            </div>

            {/* Card Detail */}
            <div className="flex-1">
              <CardDetail cardId={selectedCardId} />
            </div>
          </>
        )}

        {viewMode === 'review' && currentReviewCard && (
          <div className="flex-1">
            <ReviewCard cardId={currentReviewCard.id} onComplete={handleReviewComplete} />
          </div>
        )}

        {viewMode === 'stats' && (
          <div className="flex-1 p-6">
            <h1 className="text-2xl font-bold mb-6">Learning Statistics</h1>
            <LearningStatsPanel className="max-w-2xl" />
          </div>
        )}
      </div>
    </div>
  );
}

export default VocabularyLearning;
