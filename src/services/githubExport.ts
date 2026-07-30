// GitHub Export Service

import { invokeOrThrow } from './invoke';

export interface GitHubExportResult {
  totalWords: number;
  shardsCreated: number;
  outputDir: string;
}

export async function exportForGithub(
  outputDir: string,
  maxRank?: number,
): Promise<GitHubExportResult> {
  return invokeOrThrow('export_for_github', { outputDir, maxRank });
}

export async function exportAiCacheForGithub(outputDir: string, limit?: number): Promise<number> {
  return invokeOrThrow('export_ai_cache_for_github', { outputDir, limit });
}
