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
    title: 'settings.enginePage.sectionLlm',
    description: 'settings.enginePage.sectionLlmDesc',
  },
  {
    id: 'official',
    title: 'settings.enginePage.sectionOfficial',
    description: 'settings.enginePage.sectionOfficialDesc',
  },
  {
    id: 'web',
    title: 'settings.enginePage.sectionWeb',
    description: 'settings.enginePage.sectionWebDesc',
  },
  {
    id: 'offline',
    title: 'settings.enginePage.sectionOffline',
    description: 'settings.enginePage.sectionOfflineDesc',
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
  section: EngineSectionId;
  free?: boolean;
  needsCredentials: boolean;
  /** Short note for status badge */
  credentialHint?: string;
}

export const ENGINE_META: EngineMeta[] = [
  {
    id: 'llm',
    section: 'llm',
    needsCredentials: true,
    credentialHint: '需要 API Key',
  },
  { id: 'google', section: 'official', free: true, needsCredentials: false },
  {
    id: 'youdao',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '免配置网页接口',
  },
  {
    id: 'caiyun',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 Token',
  },
  {
    id: 'deepl',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 API Key',
  },
  {
    id: 'deeplx',
    section: 'official',
    free: true,
    needsCredentials: false,
    credentialHint: '可选自建 Key',
  },
  {
    id: 'baidu',
    section: 'official',
    needsCredentials: true,
    credentialHint: '需要 AppId',
  },
  {
    id: 'microsoft',
    section: 'official',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'yandex',
    section: 'official',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'offline',
    section: 'offline',
    free: true,
    needsCredentials: false,
  },
  {
    id: 'tatoeba',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '例句查询，非机翻',
  },
  {
    id: 'baidu_web',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'caiyun_web',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'volcengine_web',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'transmart',
    section: 'web',
    free: true,
    needsCredentials: false,
    credentialHint: '非常规，可能失效',
  },
  {
    id: 'papago',
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
