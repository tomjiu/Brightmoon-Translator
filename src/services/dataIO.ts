// Data IO Service - 学习数据导入导出 API

import { invokeOrThrow } from './invoke';

export interface ExportCard {
  word: string;
  fsrsState: unknown;
  aiContent?: string;
  createdAt: number;
  eventCount: number;
  lastReview?: number;
}

export interface DailyActivityRow {
  date: string;
  newCards: number;
  reviewedCards: number;
}

export interface ExportData {
  version: string;
  exportedAt: number;
  totalCards: number;
  cards: ExportCard[];
  dailyActivity: DailyActivityRow[];
}

export interface AnkiNote {
  front: string;
  back: string;
  tags: string[];
  interval: number;
  easeFactor: number;
  reps: number;
  lapses: number;
}

export interface ImportResult {
  imported: number;
  skipped: number;
  total: number;
}

export async function exportLearningDataJson(): Promise<ExportData> {
  return invokeOrThrow('export_learning_data_json');
}

export async function exportAnkiTsv(): Promise<AnkiNote[]> {
  return invokeOrThrow('export_anki_tsv');
}

export async function importLearningDataJson(filePath: string): Promise<ImportResult> {
  return invokeOrThrow('import_learning_data_json', { filePath });
}

export async function importWordlistCsv(filePath: string): Promise<ImportResult> {
  return invokeOrThrow('import_wordlist_csv', { filePath });
}

export async function autoBackup(backupDir: string): Promise<string> {
  return invokeOrThrow('auto_backup', { backupDir });
}

/// 将 Anki Note 列表转为 TSV 文本
export function ankiNotesToTsv(notes: AnkiNote[]): string {
  const header = '#separator:tab\n#html:true\n#tags column:5\n';
  const rows = notes.map((note) => [note.front, note.back, '', '', note.tags.join(' ')].join('\t'));
  return header + rows.join('\n');
}

export async function writeFileContent(filePath: string, content: string): Promise<void> {
  return invokeOrThrow('write_file_content', { filePath, content });
}
