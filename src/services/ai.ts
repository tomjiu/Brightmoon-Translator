import { invokeOrThrow } from './invoke';

export type PolishStyle = 'natural' | 'formal' | 'casual' | 'technical' | 'literary';

export interface PolishRequest {
  sourceText: string;
  translatedText: string;
  fromLang: string;
  toLang: string;
  style: PolishStyle;
}

export interface ExtractTermsRequest {
  texts: Array<[string, string]>;
  fromLang: string;
  toLang: string;
}

export interface AiTermEntry {
  source: string;
  target: string;
  context: string | null;
  frequency: number;
  confidence: number;
}

export interface LearnStyleRequest {
  history: Array<[string, string]>;
  fromLang: string;
  toLang: string;
}

export interface TranslationStyle {
  vocabularyLevel: string;
  sentenceStructure: string;
  tone: string;
  formality: string;
  examples: StyleExample[];
}

export interface StyleExample {
  source: string;
  translation: string;
}

export interface MultiRoundRequest {
  text: string;
  fromLang: string;
  toLang: string;
  rounds: number;
}

export interface TranslationRound {
  index: number;
  translation: string;
  qualityScore: number;
}

export interface MultiRoundResult {
  rounds: TranslationRound[];
  bestIndex: number;
  bestTranslation: string;
}

/**
 * Polish translation with AI enhancement
 */
export async function aiPolishTranslation(request: PolishRequest): Promise<string> {
  return invokeOrThrow<string>('ai_polish_translation', { request });
}

/**
 * Extract terms from translation pairs
 */
export async function aiExtractTerms(request: ExtractTermsRequest): Promise<AiTermEntry[]> {
  return invokeOrThrow<AiTermEntry[]>('ai_extract_terms', { request });
}

/**
 * Learn translation style from history
 */
export async function aiLearnStyle(request: LearnStyleRequest): Promise<TranslationStyle> {
  return invokeOrThrow<TranslationStyle>('ai_learn_style', { request });
}

/**
 * Multi-round translation optimization
 */
export async function aiMultiRoundTranslate(request: MultiRoundRequest): Promise<MultiRoundResult> {
  return invokeOrThrow<MultiRoundResult>('ai_multi_round_translate', { request });
}
