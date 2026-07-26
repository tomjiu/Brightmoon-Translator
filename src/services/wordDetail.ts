// Word Detail Service - 单词详情增强服务

import { invoke } from '@tauri-apps/api/core';

export interface WordHistory {
  eventType: string;
  timestamp: number;
  rating?: string;
  difficulty?: number;
  stability?: number;
  nextReview?: number;
}

export interface FsrsTimeline {
  date: string;
  difficulty: number;
  stability: number;
}

export interface RelatedWord {
  word: string;
  relationType: string; // root, synonym, antonym
  definition?: string;
}

export interface AiContent {
  etymology?: string;
  mnemonics?: string;
  examples?: string[];
  tips?: string;
}

export async function getWordHistory(word: string): Promise<WordHistory[]> {
  return invoke('get_word_history', { word });
}

export async function getFsrsTimeline(word: string): Promise<FsrsTimeline[]> {
  return invoke('get_fsrs_timeline', { word });
}

export async function updateAiContent(word: string, aiContent: AiContent): Promise<void> {
  return invoke('update_ai_content', { word, aiContent });
}

export async function getRelatedWords(word: string): Promise<RelatedWord[]> {
  return invoke('get_related_words', { word });
}

export async function getCorpusExamples(word: string, limit: number): Promise<string[]> {
  return invoke('get_corpus_examples', { word, limit });
}

export async function getWordEtymology(word: string): Promise<string> {
  return invoke('get_word_etymology', { word });
}
