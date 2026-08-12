import { invokeOrThrow } from './invoke';

/** Registry-driven language-pair model info (mirrors backend OfflineModelInfo). */
export interface OfflineModelInfo {
  id: string;
  from: string;
  to: string;
  displayName: string;
  sizeLabel: string;
  sizeBytes: number;
  downloaded: boolean;
  sha256: string;
}

/** One leg of a translation chain (direct pair or English pivot). */
export interface OfflineChainPair {
  id: string;
  displayName: string;
  downloaded: boolean;
}

/** Resolved chain for a pair; direct=false means it pivots through English. */
export interface OfflineChainInfo {
  from: string;
  to: string;
  direct: boolean;
  pairs: OfflineChainPair[];
}

/** Offline engine status (mirrors backend get_offline_status payload). */
export interface OfflineStatus {
  enabled: boolean;
  autoSwitch: boolean;
  loadedModels: string[];
  modelDir: string;
}

/** List all downloadable model pairs with their download state. */
export function getOfflineModels(): Promise<OfflineModelInfo[]> {
  return invokeOrThrow<OfflineModelInfo[]>('get_offline_models');
}

/** Resolve which models a language pair needs (direct or English pivot). */
export function getOfflineChain(
  from: string,
  to: string,
): Promise<OfflineChainInfo | null> {
  return invokeOrThrow<OfflineChainInfo | null>('get_offline_chain', { from, to });
}

/** Download a model pair by `from`/`to`; progress arrives via Tauri events. */
export async function downloadOfflineModel(from: string, to: string): Promise<void> {
  await invokeOrThrow<undefined>('download_offline_model', { from, to });
}

/** Delete a downloaded model pair by `from`/`to`. */
export async function deleteOfflineModel(from: string, to: string): Promise<void> {
  await invokeOrThrow<undefined>('delete_offline_model', { from, to });
}

/** Get offline engine status (enabled/autoSwitch/loaded pairs/model dir). */
export function getOfflineStatus(): Promise<OfflineStatus> {
  return invokeOrThrow<OfflineStatus>('get_offline_status');
}
