// FSRS Optimization Service - FSRS 算法优化 API

import { invokeOrThrow } from './invoke';

export interface FsrsAnalysis {
  currentParams: number[];
  retentionRate: number;
  avgIntervalDays: number;
  avgDifficulty: number;
  avgStability: number;
  totalLapses: number;
  optimalParams?: number[];
}

export interface ForgettingCurvePoint {
  days: number;
  retention: number;
}

export interface ReviewForecast {
  date: string;
  dueCount: number;
}

export interface StudyTimeSlot {
  hour: number;
  label: string;
  correctRate: number;
  reviewCount: number;
}

export interface DifficultyBucket {
  rangeStart: number;
  rangeEnd: number;
  count: number;
}

export interface ApplyParamsResult {
  applied: boolean;
  params: number[];
  reason: string;
}

export async function getFsrsAnalysis(): Promise<FsrsAnalysis> {
  return invokeOrThrow('get_fsrs_analysis');
}

export async function applyFsrsParams(): Promise<ApplyParamsResult> {
  return invokeOrThrow('apply_fsrs_params');
}

export async function resetFsrsParams(): Promise<void> {
  return invokeOrThrow('reset_fsrs_params');
}

export async function getForgettingCurve(stability: number): Promise<ForgettingCurvePoint[]> {
  return invokeOrThrow('get_forgetting_curve', { stability });
}

export async function getReviewForecast(days: number): Promise<ReviewForecast[]> {
  return invokeOrThrow('get_review_forecast', { days });
}

export async function getBestStudyTime(): Promise<StudyTimeSlot[]> {
  return invokeOrThrow('get_best_study_time');
}

export async function getDifficultyDistribution(): Promise<DifficultyBucket[]> {
  return invokeOrThrow('get_difficulty_distribution');
}
