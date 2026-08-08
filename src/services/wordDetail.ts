// Word Detail Service - 单词详情增强服务

import { invokeOrThrow } from './invoke';

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

export interface RootGraph {
  word: string;
  roots: string[];
  rootMates: RelatedWord[];
  source: string;
}

export interface AiContent {
  etymology?: string;
  mnemonics?: string;
  examples?: string[];
  tips?: string;
}

export async function getWordHistory(word: string): Promise<WordHistory[]> {
  return invokeOrThrow('get_word_history', { word });
}

export async function getFsrsTimeline(word: string): Promise<FsrsTimeline[]> {
  return invokeOrThrow('get_fsrs_timeline', { word });
}

export async function updateAiContent(word: string, aiContent: AiContent): Promise<void> {
  return invokeOrThrow('update_ai_content', { word, aiContent });
}

export async function getRelatedWords(word: string): Promise<RelatedWord[]> {
  return invokeOrThrow('get_related_words', { word });
}

export async function getRootGraph(word: string): Promise<RootGraph> {
  return invokeOrThrow('get_root_graph', { word });
}

export async function getCorpusExamples(word: string, limit: number): Promise<string[]> {
  return invokeOrThrow('get_corpus_examples', { word, limit });
}

export async function getWordEtymology(word: string): Promise<string> {
  return invokeOrThrow('get_word_etymology', { word });
}
