export type EngineId =
  | 'llm'
  | 'google'
  | 'youdao'
  | 'caiyun'
  | 'deepl'
  | 'deeplx'
  | 'baidu'
  | 'microsoft'
  | 'yandex'
  | 'offline';

export const DEFAULT_ENGINE_ORDER: EngineId[] = [
  'llm',
  'google',
  'youdao',
  'caiyun',
  'deepl',
  'deeplx',
  'baidu',
  'microsoft',
  'yandex',
  'offline',
];

export interface EngineMeta {
  id: EngineId;
  nameZh: string;
  free?: boolean;
  needsCredentials: boolean;
  /** Short note for status badge */
  credentialHint?: string;
}

export const ENGINE_META: EngineMeta[] = [
  { id: 'llm', nameZh: 'LLM 大模型翻译', needsCredentials: true, credentialHint: '需要 API Key' },
  { id: 'google', nameZh: 'Google 翻译', free: true, needsCredentials: false },
  { id: 'youdao', nameZh: '有道翻译', free: true, needsCredentials: false },
  { id: 'caiyun', nameZh: '彩云小译', needsCredentials: true, credentialHint: '需要 Token' },
  { id: 'deepl', nameZh: 'DeepL', needsCredentials: true, credentialHint: '需要 API Key' },
  {
    id: 'deeplx',
    nameZh: 'DeepLX',
    free: true,
    needsCredentials: false,
    credentialHint: '可选自建 Key',
  },
  { id: 'baidu', nameZh: '百度翻译', needsCredentials: true, credentialHint: '需要 AppId' },
  { id: 'microsoft', nameZh: 'Microsoft 翻译', free: true, needsCredentials: false },
  { id: 'yandex', nameZh: 'Yandex 翻译', free: true, needsCredentials: false },
  { id: 'offline', nameZh: '离线翻译', free: true, needsCredentials: false },
];

export function isLlmConfigured(llm: { apiKey?: string; apiKeys?: string[] }): boolean {
  if (llm.apiKey?.trim()) return true;
  return (llm.apiKeys || []).some((k) => k.trim().length > 0);
}
