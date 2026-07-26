// FSRS Optimization Service - FSRS 算法优化 API

import { invoke } from '@tauri-apps/api/core';

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

export async function getFsrsAnalysis(): Promise<FsrsAnalysis> {
  return invoke('get_fsrs_analysis');
}

export async function getForgettingCurve(stability: number): Promise<ForgettingCurvePoint[]> {
  return invoke('get_forgetting_curve', { stability });
}

export async function getReviewForecast(days: number): Promise<ReviewForecast[]> {
  return invoke('get_review_forecast', { days });
}

export async function getBestStudyTime(): Promise<StudyTimeSlot[]> {
  return invoke('get_best_study_time');
}

export async function getDifficultyDistribution(): Promise<DifficultyBucket[]> {
  return invoke('get_difficulty_distribution');
}
