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

export async function getEcDictDownloadInfo(): Promise<EcDictDownloadInfo> {
  return invokeOrThrow<EcDictDownloadInfo>('ecdict_download_info');
}

export async function downloadEcDict(): Promise<string> {
  return invokeOrThrow<string>('download_ecdict');
}

export function readEcDictInfoSilently(): Promise<EcDictDownloadInfo | null> {
  return safeInvoke<EcDictDownloadInfo>('ecdict_download_info', undefined, {
    silent: true,
  }).then(([data]) => data);
}