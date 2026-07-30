// Statistics Service - 学习统计 API

import { invokeOrThrow } from './invoke';

export interface LearningStatistics {
  totalCards: number;
  dueCards: number;
  learnedToday: number;
  reviewedToday: number;
  streakDays: number;
  totalReviews: number;
  retentionRate: number;
  avgDailyNew: number;
  avgDailyReview: number;
}

export interface DailyActivity {
  date: string; // YYYY-MM-DD
  newCards: number;
  reviewedCards: number;
  timeSpent: number; // seconds
  correctRate: number;
}

export interface HeatmapData {
  date: string; // YYYY-MM-DD
  count: number;
}

export interface WeakWord {
  word: string;
  againCount: number;
  totalReviews: number;
  lastReview: number;
  difficulty: number;
  stability: number;
}

export async function getLearningStatistics(): Promise<LearningStatistics> {
  return invokeOrThrow('get_learning_statistics');
}

export async function getDailyActivity(days: number): Promise<DailyActivity[]> {
  return invokeOrThrow('get_daily_activity', { days });
}

export async function getHeatmapData(year: number): Promise<HeatmapData[]> {
  return invokeOrThrow('get_heatmap_data', { year });
}

export async function getWeakWords(limit: number): Promise<WeakWord[]> {
  return invokeOrThrow('get_weak_words', { limit });
}
