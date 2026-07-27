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
  | 'offline'
  | 'tatoeba'
  | 'baidu_web'
  | 'caiyun_web'
  | 'volcengine_web'
  | 'transmart'
  | 'papago';

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
  'tatoeba',
  'baidu_web',
  'caiyun_web',
  'volcengine_web',
  'transmart',
  'papago',
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
  {
    id: 'youdao',
    nameZh: '有道翻译',
    free: true,
    needsCredentials: false,
    credentialHint: '免配置网页接口',
  },
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
  {
    id: 'tatoeba',
    nameZh: 'Tatoeba 例句',
    free: true,
    needsCredentials: false,
    credentialHint: '例句查询，非机翻',
  },
  {
    id: 'baidu_web',
    nameZh: '百度（免配置）',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'caiyun_web',
    nameZh: '彩云（免配置）',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'volcengine_web',
    nameZh: '火山（免配置）',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'transmart',
    nameZh: '腾讯交互翻译',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'papago',
    nameZh: 'Papago',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
];

export function isLlmConfigured(llm: { apiKey?: string; apiKeys?: string[] }): boolean {
  if (llm.apiKey?.trim()) return true;
  return (llm.apiKeys || []).some((k) => k.trim().length > 0);
}
