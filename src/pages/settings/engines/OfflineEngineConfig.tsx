import { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useI18n } from '../../../i18n';
import type { EngineConfigProps } from './types';
import {
  deleteOfflineModel,
  downloadOfflineModel,
  getOfflineModels,
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

      <label className="flex items-center gap-2">
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
          className="rounded"
        />
        <span className="ui-body text-text-secondary">{t('settings.enginePage.autoOffline')}</span>
      </label>
    </div>
  );
}
