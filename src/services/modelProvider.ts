// Model Provider Service - 模型提供商管理（openai / anthropic / gemini）

import { invokeOrThrow } from './invoke';

export type LlmApiFormat = 'openai' | 'anthropic' | 'gemini';

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
  /** Wire format for chat + model list. Mirrors Rust `api_format` (non-optional, defaults to "openai"). */
  apiFormat: LlmApiFormat;
}

export interface ProviderPreset {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  priority: number;
  apiFormat: LlmApiFormat;
}

// 内置预设：选模板 → 填 Key → 拉模型
export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: 'deepseek',
    name: 'DeepSeek',
    baseUrl: 'https://api.deepseek.com/v1',
    model: 'deepseek-chat',
    priority: 0,
    apiFormat: 'openai',
  },
  {
    id: 'siliconflow',
    name: 'SiliconFlow',
    baseUrl: 'https://api.siliconflow.cn/v1',
    model: 'Qwen/Qwen2.5-7B-Instruct',
    priority: 1,
    apiFormat: 'openai',
  },
  {
    id: 'openai',
    name: 'OpenAI',
    baseUrl: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    priority: 2,
    apiFormat: 'openai',
  },
  {
    id: 'openrouter',
    name: 'OpenRouter',
    baseUrl: 'https://openrouter.ai/api/v1',
    model: '',
    priority: 3,
    apiFormat: 'openai',
  },
  {
    id: 'moonshot',
    name: 'Moonshot/Kimi',
    baseUrl: 'https://api.moonshot.cn/v1',
    model: 'moonshot-v1-8k',
    priority: 4,
    apiFormat: 'openai',
  },
  {
    id: 'zhipu',
    name: '智谱',
    baseUrl: 'https://open.bigmodel.cn/api/paas/v4',
    model: 'glm-4-flash',
    priority: 5,
    apiFormat: 'openai',
  },
  {
    id: 'qwen',
    name: '通义千问',
    baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    model: 'qwen-plus',
    priority: 6,
    apiFormat: 'openai',
  },
  {
    id: 'groq',
    name: 'Groq',
    baseUrl: 'https://api.groq.com/openai/v1',
    model: 'llama-3.3-70b-versatile',
    priority: 7,
    apiFormat: 'openai',
  },
  {
    id: 'together',
    name: 'Together',
    baseUrl: 'https://api.together.xyz/v1',
    model: '',
    priority: 8,
    apiFormat: 'openai',
  },
  {
    id: 'ollama',
    name: 'Ollama (本地)',
    baseUrl: 'http://localhost:11434/v1',
    model: 'qwen2.5:7b',
    priority: 9,
    apiFormat: 'openai',
  },
  {
    id: 'anthropic',
    name: 'Anthropic Claude',
    baseUrl: 'https://api.anthropic.com/v1',
    model: 'claude-sonnet-4-0',
    priority: 10,
    apiFormat: 'anthropic',
  },
  {
    id: 'gemini',
    name: 'Google Gemini',
    baseUrl: 'https://generativelanguage.googleapis.com/v1beta',
    model: 'gemini-2.0-flash',
    priority: 11,
    apiFormat: 'gemini',
  },
];

export async function fetchAvailableModels(
  baseUrl: string,
  apiKey: string,
  apiFormat?: LlmApiFormat,
): Promise<ModelInfo[]> {
  return invokeOrThrow('fetch_available_models', {
    baseUrl,
    apiKey,
    apiFormat: apiFormat || 'openai',
  });
}

export async function testLlmConnection(
  baseUrl: string,
  apiKey: string,
  model: string,
  apiFormat?: LlmApiFormat,
): Promise<string> {
  return invokeOrThrow('test_llm_connection', {
    baseUrl,
    apiKey,
    model,
    apiFormat: apiFormat || 'openai',
  });
}
