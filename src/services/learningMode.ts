// Learning Mode Service - 多样化学习模式 API

import { invokeOrThrow } from './invoke';
import type { CardInfo } from './vocabulary';

export interface ChoiceQuestion {
  word: string;
  question: string;
  options: string[];
  correctIndex: number;
  explanation?: string;
}

export interface SpellingQuestion {
  definition: string;
  hint: string;
  answer: string;
  example?: string;
}

export interface ClozeQuestion {
  sentence: string;
  answer: string;
  options: string[];
  context?: string;
}

export async function generateChoiceQuestions(
  planId: string | null,
  count: number,
): Promise<ChoiceQuestion[]> {
  return invokeOrThrow('generate_choice_questions', { planId, count });
}

export async function generateSpellingQuestions(
  planId: string | null,
  count: number,
): Promise<SpellingQuestion[]> {
  return invokeOrThrow('generate_spelling_questions', { planId, count });
}

export async function generateClozeQuestions(
  planId: string | null,
  count: number,
): Promise<ClozeQuestion[]> {
  return invokeOrThrow('generate_cloze_questions', { planId, count });
}

export async function getSwipeCards(count: number): Promise<CardInfo[]> {
  return invokeOrThrow('get_swipe_cards', { count });
}

export async function submitSwipeRating(
  cardId: string,
  rating: 'Again' | 'Hard' | 'Good' | 'Easy',
): Promise<void> {
  return invokeOrThrow('submit_swipe_rating', { cardId, rating: rating.toLowerCase() });
}
