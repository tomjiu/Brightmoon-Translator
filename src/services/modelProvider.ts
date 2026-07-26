// Model Provider Service - 模型提供商管理

import { invoke } from '@tauri-apps/api/core';

export interface ModelInfo {
  id: string;
  name: string;
  ownedBy?: string;
}

export interface LlmProviderEntry {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  priority: number;
  enabled: boolean;
  models: string[];
}

// 预设提供商配置
export const PROVIDER_PRESETS: Array<
  Omit<LlmProviderEntry, 'id' | 'apiKey' | 'enabled' | 'models'>
> = [
  {
    name: 'DeepSeek',
    baseUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    priority: 0,
  },
  {
    name: 'SiliconFlow',
    baseUrl: 'https://api.siliconflow.cn/v1',
    model: 'Qwen/Qwen3.5-35B-A3B',
    priority: 1,
  },
  {
    name: 'OpenAI',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    priority: 2,
  },
  {
    name: 'Ollama (本地)',
    baseUrl: 'http://localhost:11434/v1',
    model: 'qwen2.5:7b',
    priority: 3,
  },
];

export async function fetchAvailableModels(baseUrl: string, apiKey: string): Promise<ModelInfo[]> {
  return invoke('fetch_available_models', { baseUrl, apiKey });
}

export async function testLlmConnection(
  baseUrl: string,
  apiKey: string,
  model: string,
): Promise<string> {
  return invoke('test_llm_connection', { baseUrl, apiKey, model });
}
