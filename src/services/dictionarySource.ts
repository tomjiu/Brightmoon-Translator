// Dictionary Source Service - T7 可插拔词典源

import { invokeOrThrow } from './invoke';

export interface DictSourceConfig {
  id: string;
  name: string;
  enabled: boolean;
  priority: number;
  prompt_template?: string;
}

export interface DictEntryResult {
  word: string;
  phonetics: string[];
  chinese_translation?: string;
  english_definitions: string[];
  pos: string[];
  examples: string[];
  source: string;
  raw?: string;
}

export interface SaveSourceRequest {
  source_id: string;
  enabled: boolean;
  priority: number;
  prompt_template?: string;
}

/**
 * 列出所有词典源配置
 */
export async function getDictSources(): Promise<DictSourceConfig[]> {
  return await invokeOrThrow<DictSourceConfig[]>('get_dict_sources');
}

/**
 * 更新单个词典源配置
 */
export async function updateDictSource(
  sourceId: string,
  options: {
    enabled?: boolean;
    priority?: number;
    promptTemplate?: string;
  },
): Promise<undefined> {
  return await invokeOrThrow('update_dict_source', {
    sourceId,
    enabled: options.enabled,
    priority: options.priority,
    promptTemplate: options.promptTemplate,
  });
}

/**
 * 聚合查询所有启用源
 */
export async function lookupWordAllSources(word: string): Promise<DictEntryResult[]> {
  return await invokeOrThrow<DictEntryResult[]>('lookup_word_all_sources', { word });
}

/**
 * 批量保存源配置
 */
export async function saveDictSources(sources: SaveSourceRequest[]): Promise<undefined> {
  return await invokeOrThrow('save_dict_sources', { sources });
}
