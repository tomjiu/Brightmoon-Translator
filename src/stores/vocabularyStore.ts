// Vocabulary Store - 全局词汇学习状态管理

import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { CardInfo, LearningPhase } from '../services/vocabulary';

// ============================================
// 类型定义
// ============================================

interface VocabularyState {
  // 当前学习的卡牌
  currentCard: CardInfo | null;
  setCurrentCard: (card: CardInfo | null) => void;

  // 学习会话
  sessionStartTime: number | null;
  sessionCardsReviewed: number;
  sessionCardsCorrect: number;
  startSession: () => void;
  endSession: () => void;
  incrementReviewed: (correct: boolean) => void;

  // 筛选和排序
  phaseFilter: LearningPhase | 'all';
  setPhaseFilter: (phase: LearningPhase | 'all') => void;

  // UI 状态
  isReviewMode: boolean;
  setReviewMode: (mode: boolean) => void;

  // 偏好设置
  autoPlayAudio: boolean;
  setAutoPlayAudio: (enabled: boolean) => void;
  showPhonetic: boolean;
  setShowPhonetic: (show: boolean) => void;

  // 重置状态
  reset: () => void;
}

// ============================================
// Store
// ============================================

export const useVocabularyStore = create<VocabularyState>()(
  persist(
    (set) => ({
      // 当前卡牌
      currentCard: null,
      setCurrentCard: (card) => set({ currentCard: card }),

      // 学习会话
      sessionStartTime: null,
      sessionCardsReviewed: 0,
      sessionCardsCorrect: 0,
      startSession: () =>
        set({
          sessionStartTime: Date.now(),
          sessionCardsReviewed: 0,
          sessionCardsCorrect: 0,
        }),
      endSession: () =>
        set({
          sessionStartTime: null,
        }),
      incrementReviewed: (correct) =>
        set((state) => ({
          sessionCardsReviewed: state.sessionCardsReviewed + 1,
          sessionCardsCorrect: correct ? state.sessionCardsCorrect + 1 : state.sessionCardsCorrect,
        })),

      // 筛选和排序
      phaseFilter: 'all',
      setPhaseFilter: (phase) => set({ phaseFilter: phase }),

      // UI 状态
      isReviewMode: false,
      setReviewMode: (mode) => set({ isReviewMode: mode }),

      // 偏好设置
      autoPlayAudio: false,
      setAutoPlayAudio: (enabled) => set({ autoPlayAudio: enabled }),
      showPhonetic: true,
      setShowPhonetic: (show) => set({ showPhonetic: show }),

      // 重置状态
      reset: () =>
        set({
          currentCard: null,
          sessionStartTime: null,
          sessionCardsReviewed: 0,
          sessionCardsCorrect: 0,
          phaseFilter: 'all',
          isReviewMode: false,
        }),
    }),
    {
      name: 'vocabulary-storage',
      partialize: (state) => ({
        // 只持久化偏好设置
        autoPlayAudio: state.autoPlayAudio,
        showPhonetic: state.showPhonetic,
        phaseFilter: state.phaseFilter,
      }),
    },
  ),
);

// ============================================
// Selectors
// ============================================

export const selectSessionStats = (state: VocabularyState) => {
  const { sessionStartTime, sessionCardsReviewed, sessionCardsCorrect } = state;

  if (!sessionStartTime) {
    return null;
  }

  const duration = Date.now() - sessionStartTime;
  const accuracy =
    sessionCardsReviewed > 0 ? (sessionCardsCorrect / sessionCardsReviewed) * 100 : 0;

  return {
    duration,
    cardsReviewed: sessionCardsReviewed,
    cardsCorrect: sessionCardsCorrect,
    accuracy,
  };
};

export const selectIsSessionActive = (state: VocabularyState) => state.sessionStartTime !== null;
