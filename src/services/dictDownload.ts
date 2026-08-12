// Dictionary cloud download service (ecdict.db)

import { invokeOrThrow, safeInvoke } from './invoke';

export interface EcDictDownloadInfo {
  present: boolean;
  length: number;
  path?: string;
}

export interface EcDictProgress {
  received: number;
  total: number;
  percent: number;
}

export interface DictVariant {
  id: string;
  label: string;
  detail: string;
  sizeHuman: string;
  sizeBytes: number;
  url: string;
  sha256: string;
}

const BASE = 'https://github.com/tomjiu/Brightmoon-Translator/releases/download/ecdict-v1';

export const DICT_VARIANTS: DictVariant[] = [
  {
    id: 'compact',
    label: '精简版',
    detail: '高频词（frq≤8000，81 万词条），满足日常查询与学习',
    sizeHuman: '106 MB',
    sizeBytes: 106 * 1024 * 1024,
    url: `${BASE}/ecdict-compact-8k.db`,
    sha256: 'eabf17bd8553a10ca3ff0182c5a159570bc93d7092a4284db2b54d98deb4f8ac',
  },
  {
    id: 'full',
    label: '完整版',
    detail: 'ECDICT 全量（340 万词条），含低频词与扩展字段',
    sizeHuman: '812 MB',
    sizeBytes: 812 * 1024 * 1024,
    url: `${BASE}/ecdict.db`,
    sha256: '2b5b40c2bdba04da0a51c8672e090f166987d5d895f32eb3fbfc5a516455fc75',
  },
];

export async function getEcDictDownloadInfo(): Promise<EcDictDownloadInfo> {
  return invokeOrThrow<EcDictDownloadInfo>('ecdict_download_info');
}

export async function downloadEcDict(variant: DictVariant): Promise<string> {
  return invokeOrThrow<string>('download_ecdict', {
    url: variant.url,
    expectedSha256: variant.sha256,
  });
}

export function readEcDictInfoSilently(): Promise<EcDictDownloadInfo | null> {
  return safeInvoke<EcDictDownloadInfo>('ecdict_download_info', undefined, {
    silent: true,
  }).then(([data]) => data);
}