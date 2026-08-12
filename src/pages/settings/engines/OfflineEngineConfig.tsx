import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';
import { LANGUAGES } from '../../../types';
import {
  deleteOfflineModel,
  downloadOfflineModel,
  getOfflineModels,
  getOfflineChain,
  type OfflineChainInfo,
  type OfflineModelInfo,
} from '../../../services/offline';

/** Mirrors backend `offline-download-progress` event payload. */
interface DownloadProgressEvent {
  pair: string;
  file: number;
  fileTotal: number;
  done: number;
  total: number;
}

export default function OfflineEngineConfig({
  config,
  updateConfig,
  saveConfig,
}: EngineConfigProps) {
  const { t } = useI18n();
  const offline = config.engines.offline;

  const [models, setModels] = useState<OfflineModelInfo[]>([]);
  const [busy, setBusy] = useState<ReadonlySet<string>>(new Set());
  const [progress, setProgress] = useState<Record<string, number>>({});

  const [chainFrom, setChainFrom] = useState('en');
  const [chainTo, setChainTo] = useState('zh');
  const [chainInfo, setChainInfo] = useState<OfflineChainInfo | null>(null);
  const [chainLoading, setChainLoading] = useState(false);

  const handleCheckChain = useCallback(async () => {
    if (!chainFrom || !chainTo || chainFrom === chainTo) return;
    setChainLoading(true);
    try {
      setChainInfo(await getOfflineChain(chainFrom, chainTo));
    } catch {
      setChainInfo(null);
    } finally {
      setChainLoading(false);
    }
  }, [chainFrom, chainTo]);

  const refresh = useCallback(async () => {
    try {
      setModels(await getOfflineModels());
    } catch {
      // error toast is handled by invokeOrThrow
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<DownloadProgressEvent>('offline-download-progress', (event) => {
      const { pair, done, total } = event.payload;
      const percent = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
      setProgress((prev) => ({ ...prev, [pair]: percent }));
      if (percent >= 100) {
        setBusy((prev) => {
          const next = new Set(prev);
          next.delete(pair);
          return next;
        });
        void refresh();
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refresh]);

  const downloadedCount = models.filter((m) => m.downloaded).length;

  const handleDownload = async (m: OfflineModelInfo) => {
    setBusy((prev) => new Set(prev).add(m.id));
    setProgress((prev) => ({ ...prev, [m.id]: 0 }));
    try {
      await downloadOfflineModel(m.from, m.to);
      await refresh();
    } catch {
      // error toast is handled by invokeOrThrow
    } finally {
      setBusy((prev) => {
        const next = new Set(prev);
        next.delete(m.id);
        return next;
      });
    }
  };

  const handleDelete = async (m: OfflineModelInfo) => {
    setBusy((prev) => new Set(prev).add(m.id));
    try {
      await deleteOfflineModel(m.from, m.to);
      await refresh();
    } catch {
      // error toast is handled by invokeOrThrow
    } finally {
      setBusy((prev) => {
        const next = new Set(prev);
        next.delete(m.id);
        return next;
      });
    }
  };

  return (
    <div className="mt-3 space-y-3">
      <p className="ui-caption">
        {t('settings.enginePage.downloadedModels', { count: downloadedCount })}
      </p>

      <div className="space-y-2">
        {models.map((m) => {
          const isBusy = busy.has(m.id);
          return (
            <div
              key={m.id}
              className="flex items-center justify-between gap-3 rounded-lg border border-border bg-bg-secondary p-2"
            >
              <div className="min-w-0 flex-1">
                <p className="ui-section-title truncate">{m.displayName}</p>
                <p className="ui-caption mt-0.5">
                  {m.downloaded
                    ? t('settings.enginePage.downloaded')
                    : t('settings.enginePage.notDownloaded')}{' '}
                  · {m.sizeLabel}
                </p>
                {isBusy && (
                  <div className="mt-1.5 flex w-48 h-2 bg-bg-tertiary rounded-full overflow-hidden">
                    <div
                      className="h-full bg-primary transition-all"
                      style={{ width: `${String(progress[m.id] ?? 0)}%` }}
                    />
                  </div>
                )}
              </div>
              {m.downloaded ? (
                <button
                  type="button"
                  disabled={isBusy}
                  onClick={() => void handleDelete(m)}
                  className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary disabled:opacity-50"
                >
                  {t('settings.enginePage.delete')}
                </button>
              ) : (
                <button
                  type="button"
                  disabled={isBusy}
                  onClick={() => void handleDownload(m)}
                  className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary disabled:opacity-50"
                >
                  {t('settings.enginePage.download')}
                </button>
              )}
            </div>
          );
        })}
        {models.length === 0 && (
          <p className="ui-caption">{t('settings.enginePage.noPairs')}</p>
        )}
      </div>

      <p className="ui-caption leading-relaxed">{t('settings.enginePage.pivotHint')}</p>

      <div className="mt-2 space-y-2 rounded-lg border border-border bg-bg-secondary p-3">
        <p className="ui-section-title">{t('settings.enginePage.chainTitle')}</p>
        <p className="ui-caption">{t('settings.enginePage.chainPending')}</p>
        <div className="flex items-center gap-2">
          <select
            value={chainFrom}
            onChange={(e) => setChainFrom(e.target.value)}
            className="flex-1 px-2 py-1.5 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
          >
            <option value="">—</option>
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
          <span className="text-text-secondary text-sm">→</span>
          <select
            value={chainTo}
            onChange={(e) => setChainTo(e.target.value)}
            className="flex-1 px-2 py-1.5 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
          >
            <option value="">—</option>
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={chainLoading}
            onClick={() => void handleCheckChain()}
            className="px-3 py-1.5 text-sm rounded-lg bg-primary text-primary-fg font-medium disabled:opacity-50"
          >
            {t('settings.enginePage.chainCheck')}
          </button>
        </div>

        {chainInfo && (
          <div className="space-y-1.5 pt-1">
            <p className="ui-caption">
              {chainInfo.direct
                ? t('settings.enginePage.chainDirect')
                : t('settings.enginePage.chainPivot')}
            </p>
            {chainInfo.pairs.map((pair) => (
              <div
                key={pair.id}
                className={`flex items-center justify-between gap-2 px-2 py-1.5 rounded-lg text-sm ${
                  pair.downloaded
                    ? 'bg-bg-tertiary text-text-primary'
                    : 'bg-amber-500/10 text-amber-700 dark:text-amber-400'
                }`}
              >
                <span className="font-mono">{pair.id}</span>
                <span className="ui-caption">
                  {pair.downloaded
                    ? t('settings.enginePage.downloaded')
                    : t('settings.enginePage.notDownloaded')}
                </span>
              </div>
            ))}
            {!chainInfo.direct && chainInfo.pairs.some((p) => !p.downloaded) && (
              <p className="ui-caption text-amber-700 dark:text-amber-400">
                {t('settings.enginePage.chainMissing', {
                  n: chainInfo.pairs.filter((p) => !p.downloaded).length,
                })}
              </p>
            )}
          </div>
        )}
        {chainInfo === null && !chainLoading && (
          <p className="ui-caption">{t('settings.enginePage.chainNone')}</p>
        )}
      </div>

      <div className="space-y-1.5">
        <label className="block text-sm font-medium text-text-primary">
          {t('settings.enginePage.modelDirLabel')}
        </label>
        <input
          value={offline.modelDir || ''}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                offline: { ...prev.engines.offline, modelDir: e.target.value },
              },
            }));
          }}
          onBlur={() => void saveConfig()}
          placeholder={t('settings.enginePage.modelDirPh')}
          className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm font-mono"
        />
        <p className="ui-caption">{t('settings.enginePage.modelDirHint')}</p>
      </div>

      <label className="flex items-start gap-2">
        <input
          type="checkbox"
          checked={offline.autoSwitch || false}
          onChange={(e) => {
            updateConfig((prev) => ({
              ...prev,
              engines: {
                ...prev.engines,
                offline: {
                  ...prev.engines.offline,
                  autoSwitch: e.target.checked,
                },
              },
            }));
            void saveConfig();
          }}
          className="rounded mt-0.5"
        />
        <span className="ui-body text-text-secondary">{t('settings.enginePage.autoOffline')}</span>
      </label>
      <p className="ui-caption leading-relaxed">{t('settings.enginePage.autoOfflineHint')}</p>
    </div>
  );
}
