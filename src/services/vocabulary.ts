// Vocabulary Service - 词汇学习服务层

import { invoke } from '@tauri-apps/api/core';

// ============================================
// 类型定义
// ============================================

export enum LearningPhase {
  New = 'new',
  Learning = 'learning',
  Review = 'review',
  Mastered = 'mastered',
}

export enum Rating {
  Again = 'again',
  Hard = 'hard',
  Good = 'good',
  Easy = 'easy',
}

export interface CoreVocabEntry {
  word: string;
  frequency_rank: number;
  frq?: number;
  collins?: number;
  oxford?: number;
  tag?: string;
}

export interface CardInfo {
  id: string;
  word: string;
  phase: LearningPhase;
  next_review: number;
  reps: number;
  stability: number;
}

export interface WordCard {
  id: string;
  word: string;
  current_version: number;
  base_data: BaseData;
  ai_content?: AiContent;
  fsrs_state: CardState;
  error_records: ErrorRecord[];
  annotations: Annotation[];
  created_at: number;
  updated_at: number;
}

export interface BaseData {
  phonetic?: string;
  part_of_speech?: string;
  definitions: string[];
  translation?: string;
}

export interface AiContent {
  etymology?: Etymology;
  mnemonics: Mnemonic[];
  examples: PersonalizedExample[];
  scenes: Scene[];
  collocations?: string[];
  word_family?: WordFamilyItem[];
  usage_tips?: string[];
  common_mistakes?: string[];
  synonyms?: string[];
  antonyms?: string[];
}

export interface WordFamilyItem {
  word: string;
  pos: string;
  meaning: string;
}

export interface Etymology {
  origin: string;
  root_breakdown: Root[];
  historical_usage?: string;
  cognates: string[];
}

export interface Root {
  part: string;
  meaning: string;
  examples: string[];
}

export interface Mnemonic {
  mnemonic_type: 'etymology' | 'scene' | 'homophone' | 'visual' | 'chunking' | 'comparison';
  content: string;
  score?: number;
}

export interface PersonalizedExample {
  text: string;
  context: string;
  difficulty: string;
  score?: number;
  user_feedback?: string;
}

export interface Scene {
  description: string;
  dialogue: string;
  vocabulary_usage: string;
}

export interface CardState {
  stability: number;
  difficulty: number;
  elapsed_days: number;
  scheduled_days: number;
  reps: number;
  lapses: number;
  last_review: number;
  next_review: number;
}

export interface ErrorRecord {
  error_type: string;
  description: string;
  context?: string;
  timestamp: number;
}

export interface Annotation {
  field: string;
  content: string;
  timestamp: number;
}

export interface LearningStats {
  total_cards: number;
  due_cards: number;
  learned_today: number;
  reviewed_today: number;
}

// ============================================
// Service 方法
// ============================================

/**
 * 获取核心词库列表（分页）
 */
export async function getCoreVocabulary(offset = 0, limit = 50): Promise<CoreVocabEntry[]> {
  return await invoke<CoreVocabEntry[]>('get_core_vocabulary', {
    offset,
    limit,
  });
}

/**
 * 搜索核心词库
 */
export async function searchCoreVocabulary(query: string, limit = 20): Promise<CoreVocabEntry[]> {
  return await invoke<CoreVocabEntry[]>('search_core_vocabulary', {
    query,
    limit,
  });
}

/**
 * 创建新卡牌
 */
export async function createCard(word: string): Promise<string> {
  return await invoke<string>('create_card', { word });
}

/**
 * 获取卡牌详情
 */
export async function getCard(cardId: string): Promise<WordCard> {
  return await invoke<WordCard>('get_card', { cardId });
}

/**
 * 获取待复习卡牌列表
 */
export async function getDueCards(): Promise<CardInfo[]> {
  return await invoke<CardInfo[]>('get_due_cards');
}

/**
 * AI 生成卡牌内容
 */
export async function generateCardContent(cardId: string): Promise<AiContent> {
  return await invoke<AiContent>('generate_card_content', { cardId });
}

/**
 * 提交复习结果
 */
export async function submitReview(cardId: string, rating: Rating): Promise<undefined> {
  return await invoke('submit_review', { cardId, rating });
}

/**
 * 获取学习统计
 */
export async function getLearningStats(): Promise<LearningStats> {
  return await invoke<LearningStats>('get_learning_stats');
}

// ============================================
// 工具函数
// ============================================

/**
 * 格式化时间戳为可读时间
 */
export function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString('zh-CN');
}

/**
 * 计算逾期天数
 */
export function calculateOverdueDays(nextReview: number): number {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - nextReview;
  return Math.max(0, Math.floor(diff / 86400));
}

/**
 * 获取学习阶段的显示文本
 */
export function getPhaseDisplayText(phase: LearningPhase): string {
  const texts: Record<LearningPhase, string> = {
    [LearningPhase.New]: '新词',
    [LearningPhase.Learning]: '学习中',
    [LearningPhase.Review]: '复习中',
    [LearningPhase.Mastered]: '已精通',
  };
  return texts[phase];
}

/**
 * 获取评分的显示文本
 */
export function getRatingDisplayText(rating: Rating): string {
  const texts: Record<Rating, string> = {
    [Rating.Again]: '忘记',
    [Rating.Hard]: '困难',
    [Rating.Good]: '良好',
    [Rating.Easy]: '简单',
  };
  return texts[rating];
}

/**
 * 获取评分的颜色类
 */
export function getRatingColorClass(rating: Rating): string {
  const colors: Record<Rating, string> = {
    [Rating.Again]: 'text-red-500',
    [Rating.Hard]: 'text-orange-500',
    [Rating.Good]: 'text-green-500',
    [Rating.Easy]: 'text-primary',
  };
  return colors[rating];
}

/**
 * 获取学习阶段的颜色类
 */
export function getPhaseColorClass(phase: LearningPhase): string {
  const colors: Record<LearningPhase, string> = {
    [LearningPhase.New]: 'bg-gray-100 text-gray-700',
    [LearningPhase.Learning]: 'bg-bg-tertiary text-primary',
    [LearningPhase.Review]: 'bg-yellow-100 text-yellow-700',
    [LearningPhase.Mastered]: 'bg-green-100 text-green-700',
  };
  return colors[phase];
}
