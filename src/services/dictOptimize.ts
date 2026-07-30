// Dictionary Optimization Service

import { invokeOrThrow } from './invoke';

export interface DictStats {
  totalWords: number;
  highFreqWords: number;
  midFreqWords: number;
  lowFreqWords: number;
  noFreqWords: number;
  totalSizeMb: number;
}

export interface ShardInfo {
  letter: string;
  wordCount: number;
  fileName: string;
}

export interface Manifest {
  version: string;
  createdAt: string;
  totalWords: number;
  shards: ShardInfo[];
}

export interface ExportResult {
  exportedWords: number;
  outputPath: string;
}

export async function getDictStats(): Promise<DictStats> {
  return invokeOrThrow('get_dict_stats');
}

export async function exportCompressedDict(
  outputPath: string,
  maxRank: number,
): Promise<ExportResult> {
  return invokeOrThrow('export_compressed_dict', { outputPath, maxRank });
}

export async function exportDictShards(outputDir: string): Promise<Manifest> {
  return invokeOrThrow('export_dict_shards', { outputDir });
}
