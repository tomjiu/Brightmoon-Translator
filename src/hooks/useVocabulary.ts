// Vocabulary Hooks - React Hooks for vocabulary learning

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  getCoreVocabulary,
  searchCoreVocabulary,
  createCard,
  getCard,
  getDueCards,
  generateCardContent,
  submitReview,
  getLearningStats,
  type Rating,
} from './vocabulary';

// ============================================
// Query Keys
// ============================================

export const vocabularyKeys = {
  all: ['vocabulary'] as const,
  coreVocab: (offset: number, limit: number) =>
    [...vocabularyKeys.all, 'coreVocab', offset, limit] as const,
  search: (query: string, limit: number) =>
    [...vocabularyKeys.all, 'search', query, limit] as const,
  card: (cardId: string) => [...vocabularyKeys.all, 'card', cardId] as const,
  dueCards: () => [...vocabularyKeys.all, 'dueCards'] as const,
  stats: () => [...vocabularyKeys.all, 'stats'] as const,
};

// ============================================
// Hooks
// ============================================

/**
 * 获取核心词库列表
 */
export function useCoreVocabulary(offset = 0, limit = 50) {
  return useQuery({
    queryKey: vocabularyKeys.coreVocab(offset, limit),
    queryFn: () => getCoreVocabulary(offset, limit),
  });
}

/**
 * 搜索核心词库
 */
export function useSearchCoreVocabulary(query: string, limit = 20) {
  return useQuery({
    queryKey: vocabularyKeys.search(query, limit),
    queryFn: () => searchCoreVocabulary(query, limit),
    enabled: query.length > 0,
  });
}

/**
 * 创建新卡牌
 */
export function useCreateCard() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (word: string) => createCard(word),
    onSuccess: () => {
      // 使待复习卡牌列表失效
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.dueCards() });
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.stats() });
    },
  });
}

/**
 * 获取卡牌详情
 */
export function useCard(cardId: string | null) {
  return useQuery({
    queryKey: vocabularyKeys.card(cardId || ''),
    queryFn: () => getCard(cardId!),
    enabled: !!cardId,
  });
}

/**
 * 获取待复习卡牌列表
 */
export function useDueCards() {
  return useQuery({
    queryKey: vocabularyKeys.dueCards(),
    queryFn: getDueCards,
  });
}

/**
 * AI 生成卡牌内容
 */
export function useGenerateCardContent() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (cardId: string) => generateCardContent(cardId),
    onSuccess: (_, cardId) => {
      // 使卡牌详情失效，重新获取
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.card(cardId) });
    },
  });
}

/**
 * 提交复习结果
 */
export function useSubmitReview() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ cardId, rating }: { cardId: string; rating: Rating }) =>
      submitReview(cardId, rating),
    onSuccess: (_, { cardId }) => {
      // 使相关查询失效
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.card(cardId) });
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.dueCards() });
      queryClient.invalidateQueries({ queryKey: vocabularyKeys.stats() });
    },
  });
}

/**
 * 获取学习统计
 */
export function useLearningStats() {
  return useQuery({
    queryKey: vocabularyKeys.stats(),
    queryFn: getLearningStats,
  });
}

// ============================================
// 复合 Hooks
// ============================================

/**
 * 学习新卡牌的完整流程
 */
export function useLearnCard() {
  const createCardMutation = useCreateCard();
  const generateContentMutation = useGenerateCardContent();

  const learnCard = async (word: string) => {
    // 1. 创建卡牌
    const cardId = await createCardMutation.mutateAsync(word);

    // 2. 生成内容
    const content = await generateContentMutation.mutateAsync(cardId);

    // 3. 返回卡牌ID和内容
    return { cardId, content };
  };

  return {
    learnCard,
    isLoading: createCardMutation.isPending || generateContentMutation.isPending,
    error: createCardMutation.error || generateContentMutation.error,
  };
}

/**
 * 复习卡牌并自动更新
 */
export function useReviewCard() {
  const submitReviewMutation = useSubmitReview();

  return {
    reviewCard: (cardId: string, rating: Rating) =>
      submitReviewMutation.mutateAsync({ cardId, rating }),
    isLoading: submitReviewMutation.isPending,
    error: submitReviewMutation.error,
  };
}
