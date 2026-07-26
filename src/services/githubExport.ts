// GitHub Export Service

import { invoke } from '@tauri-apps/api/core';

export interface GitHubExportResult {
  totalWords: number;
  shardsCreated: number;
  outputDir: string;
}

export async function exportForGithub(
  outputDir: string,
  maxRank?: number,
): Promise<GitHubExportResult> {
  return invoke('export_for_github', { outputDir, maxRank });
}

export async function exportAiCacheForGithub(outputDir: string, limit?: number): Promise<number> {
  return invoke('export_ai_cache_for_github', { outputDir, limit });
}
