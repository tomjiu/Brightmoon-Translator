// src/services/preference.ts - T12 双偏好闭环前端调用
import { invokeOrThrow } from './invoke';

export interface FieldPreference {
  field: string;
  avgRating: number;
  ratedCount: number;
  lastFeedback: string | null;
}

export interface InferredWeakField {
  field: string;
  strength: number;
}

export async function rateCardField(
  cardId: string,
  field: string,
  rating: number,
  feedback: string | null,
): Promise<void> {
  return invokeOrThrow('rate_card_field', { cardId, field, rating, feedback });
}

export async function getUserPreferences(): Promise<FieldPreference[]> {
  return invokeOrThrow<FieldPreference[]>('get_user_preferences');
}

export async function getInferredWeakFields(): Promise<InferredWeakField[]> {
  return invokeOrThrow<InferredWeakField[]>('get_inferred_weak_fields');
}
