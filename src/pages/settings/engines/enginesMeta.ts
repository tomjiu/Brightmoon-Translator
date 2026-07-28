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

/** Visual settings groups only — does not change Rust module layout or router ids. */
export type EngineSectionId = 'llm' | 'official' | 'web' | 'offline';

export interface EngineSectionMeta {
  id: EngineSectionId;
  title: string;
  description: string;
}

export const ENGINE_SECTIONS: EngineSectionMeta[] = [
  {
    id: 'llm',
    title: 'LLM 大模型',
    description: '在「AI 增强」配置密钥与模型；此处仅摘要与路由顺序',
  },
  {
    id: 'official',
    title: '官方引擎',
    description: '官方 API 或稳定公开服务；部分需填写密钥后才会被路由使用',
  },
  {
    id: 'web',
    title: '网页 / 非常规',
    description: '免配置网页或非官方端点，可能随时失效；默认谨慎开启',
  },
  {
    id: 'offline',
    title: '离线翻译 / OCR',
    description: '本地翻译模型；离线 OCR 后端与模型目录在「OCR 识别」中配置',
  },
];

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
  section: EngineSectionId;
  free?: boolean;
  needsCredentials: boolean;
  /** Short note for status badge */
  credentialHint?: string;
}

export const ENGINE_META: EngineMeta[] = [
  {
    id: 'llm',
    nameZh: 'LLM 大模型翻译',
    section: 'llm',
    needsCredentials: true,
    credentialHint: '需要 API Key',
  },
  { id: 'google', nameZh: 'Google 翻译', section: 'official', free: true, needsCredentials: false },
  {
    id: 'youdao',
    nameZh: '有道翻译',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '免配置网页接口',
  },
  {
    id: 'caiyun',
    nameZh: '彩云小译',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 Token',
  },
  {
    id: 'deepl',
    nameZh: 'DeepL',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 API Key',
  },
  {
    id: 'deeplx',
    nameZh: 'DeepLX',
    section: 'official',
    free: true,
    needsCredentials: false,
    credentialHint: '可选自建 Key',
  },
  {
    id: 'baidu',
    nameZh: '百度翻译',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 AppId',
  },
  {
    id: 'microsoft',
    nameZh: 'Microsoft 翻译',
    section: 'official',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'yandex',
    nameZh: 'Yandex 翻译',
    section: 'official',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'offline',
    nameZh: '离线翻译',
    section: 'offline',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'tatoeba',
    nameZh: 'Tatoeba 例句',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '例句查询，非机翻',
  },
  {
    id: 'baidu_web',
    nameZh: '百度（免配置）',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'caiyun_web',
    nameZh: '彩云（免配置）',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'volcengine_web',
    nameZh: '火山（免配置）',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'transmart',
    nameZh: '腾讯交互翻译',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'papago',
    nameZh: 'Papago',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
];

const META_BY_ID = new Map(ENGINE_META.map((m) => [m.id, m]));

export function getEngineMeta(id: string): EngineMeta | undefined {
  return META_BY_ID.get(id as EngineId);
}

export function getEngineSection(id: string): EngineSectionId | undefined {
  return getEngineMeta(id)?.section;
}

/** Engines in `order` that belong to `section`, preserving order. */
export function enginesInSection(order: string[], section: EngineSectionId): string[] {
  return order.filter((id) => getEngineSection(id) === section);
}

export function isLlmConfigured(llm: { apiKey?: string; apiKeys?: string[] }): boolean {
  if (llm.apiKey?.trim()) return true;
  return (llm.apiKeys || []).some((k) => k.trim().length > 0);
}
