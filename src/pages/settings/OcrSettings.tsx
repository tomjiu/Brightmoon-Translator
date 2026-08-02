import { CheckCircle, Monitor, Sparkles, BookOpen, Cpu, type LucideIcon } from 'lucide-react';
import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import Badge from '../../components/Badge';
import {
  OCR_WATCH_INTERVAL_DEFAULT_MS,
  OCR_WATCH_INTERVAL_MIN_MS,
} from '../../services/ocrConstants';
import { useI18n } from '../../i18n';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';

type OcrEngineId = 'auto' | 'winrt' | 'youdao' | 'tesseract' | 'rapid' | 'paddle';

interface OcrEngineOption {
  id: OcrEngineId;
  nameKey: string;
  descKey: string;
  icon: LucideIcon;
  status: 'available' | 'unavailable';
  recommended?: boolean;
  badgeKeys: Array<{ labelKey: string; variant: 'success' | 'warning' | 'error' | 'info' }>;
}

interface OcrSettingsProps {
  onNavigate?: (sectionId: string) => void;
}

export default function OcrSettings({ onNavigate }: OcrSettingsProps) {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);

  // P6: DocLayout-YOLO model state
  const [modelReady, setModelReady] = useState<boolean | null>(null);
  const [modelSize, setModelSize] = useState<number>(0);
  const [downloading, setDownloading] = useState(false);
  const [downloadPct, setDownloadPct] = useState<number>(0);
  const [downloadError, setDownloadError] = useState<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void (async () => {
      try {
        setModelReady(await invoke<boolean>('is_layout_model_ready'));
        setModelSize(await invoke<number>('layout_model_size'));
      } catch {
        setModelReady(false);
      }
      try {
        unlisten = await listen<{ percent: number }>(
          'layout-model-download-progress',
          (e) => setDownloadPct(e.payload.percent),
        );
      } catch {
        // non-tauri or listener fail — ignore
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  const handleDownloadModel = async () => {
    setDownloading(true);
    setDownloadError(null);
    setDownloadPct(0);
    try {
      await invoke<string>('download_layout_model');
      setModelReady(true);
      setModelSize(await invoke<number>('layout_model_size'));
    } catch (e) {
      setDownloadError(String(e));
    } finally {
      setDownloading(false);
    }
  };

  const handleRemoveModel = async () => {
    try {
      await invoke<boolean>('remove_layout_model');
      setModelReady(false);
      setModelSize(0);
    } catch (e) {
      setDownloadError(String(e));
    }
  };

  const ocrEngines: OcrEngineOption[] = [
    {
      id: 'winrt',
      nameKey: 'settings.ocr.winrtName',
      descKey: 'settings.ocr.winrtDesc',
      icon: Monitor,
      status: 'available',
      recommended: true,
      badgeKeys: [
        { labelKey: 'settings.ocr.badgeRec', variant: 'success' },
        { labelKey: 'settings.ocr.badgeFast', variant: 'info' },
        { labelKey: 'settings.ocr.badgeFree', variant: 'success' },
      ],
    },
    {
      id: 'auto',
      nameKey: 'settings.ocr.autoName',
      descKey: 'settings.ocr.autoDesc',
      icon: Sparkles,
      status: 'available',
      badgeKeys: [{ labelKey: 'settings.ocr.badgeSmart', variant: 'info' }],
    },
    {
      id: 'youdao',
      nameKey: 'settings.ocr.youdaoName',
      descKey: 'settings.ocr.youdaoDesc',
      icon: BookOpen,
      status: 'available',
      badgeKeys: [
        { labelKey: 'settings.ocr.badgeFree', variant: 'success' },
        { labelKey: 'settings.ocr.badgeNoConfig', variant: 'info' },
      ],
    },
    {
      id: 'tesseract',
      nameKey: 'settings.ocr.tesseractName',
      descKey: 'settings.ocr.tesseractDesc',
      icon: Cpu,
      status: 'available',
      badgeKeys: [
        { labelKey: 'settings.ocr.badgeOffline', variant: 'info' },
        { labelKey: 'settings.ocr.badgeSlow', variant: 'warning' },
      ],
    },
    {
      id: 'rapid',
      nameKey: 'settings.ocr.rapidName',
      descKey: 'settings.ocr.rapidDesc',
      icon: Cpu,
      status: 'available',
      badgeKeys: [
        { labelKey: 'settings.ocr.badgeOffline', variant: 'info' },
        { labelKey: 'settings.ocr.badgeExtModel', variant: 'warning' },
      ],
    },
    {
      id: 'paddle',
      nameKey: 'settings.ocr.paddleName',
      descKey: 'settings.ocr.paddleDesc',
      icon: Cpu,
      status: 'available',
      badgeKeys: [
        { labelKey: 'settings.ocr.badgeOffline', variant: 'info' },
        { labelKey: 'settings.ocr.badgeExtModel', variant: 'warning' },
      ],
    },
  ];

  const currentEngine = (config.ocrEngine || 'winrt') as OcrEngineId;
  const currentEngineInfo = ocrEngines.find((e) => e.id === currentEngine);
  const CurrentIcon = currentEngineInfo?.icon ?? Monitor;
  const currentName = currentEngineInfo ? t(currentEngineInfo.nameKey) : currentEngine;

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.ocr.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.ocr.pageDesc')}</p>
      </div>

      <Card title={t('settings.ocr.relationTitle')}>
        <ul className="text-xs text-text-secondary space-y-1.5 leading-relaxed list-disc pl-4">
          <li>
            <span className="text-text-primary font-medium">{t('settings.ocr.relationThisPage')}</span>
            {t('settings.ocr.relationThisPageDesc')}
          </li>
          <li>
            <span className="text-text-primary font-medium">{t('settings.ocr.relationEngines')}</span>
            {t('settings.ocr.relationEnginesDesc')}
          </li>
          <li>{t('settings.ocr.relationNote')}</li>
        </ul>
        {onNavigate && (
          <button
            type="button"
            onClick={() => onNavigate('engines')}
            className="mt-3 text-xs font-medium text-primary hover:underline"
          >
            {t('settings.ocr.goEngines')}
          </button>
        )}
      </Card>

      <Card>
        <div className="flex items-center gap-3 p-3.5 rounded-xl border border-border bg-bg-tertiary">
          <div className="w-11 h-11 rounded-xl bg-bg-secondary border border-border flex items-center justify-center text-text-primary shrink-0">
            <CurrentIcon size={22} strokeWidth={1.75} />
          </div>
          <div className="flex-1 min-w-0">
            <p className="ui-caption">{t('settings.ocr.currentEngine')}</p>
            <p className="ui-section-title mt-0.5">{currentName}</p>
          </div>
          <CheckCircle size={20} className="text-text-secondary shrink-0" />
        </div>
        {(currentEngine === 'winrt' || currentEngine === 'tesseract') && (
          <p className="ui-caption mt-3 leading-relaxed">
            {t('settings.ocr.readyHint', { engine: currentName })}
          </p>
        )}
      </Card>

      <Card title={t('settings.ocr.chooseTitle')} description={t('settings.ocr.chooseDesc')}>
        <div className="space-y-2">
          {ocrEngines.map((engine) => {
            const Icon = engine.icon;
            const active = currentEngine === engine.id;
            return (
              <label
                key={engine.id}
                className={`flex items-start gap-3 p-3.5 rounded-xl border cursor-pointer transition-colors duration-150 ${
                  active
                    ? 'border-primary bg-primary/5'
                    : engine.status === 'unavailable'
                      ? 'border-border opacity-50 cursor-not-allowed'
                      : 'border-border hover:border-border-strong'
                }`}
              >
                <input
                  type="radio"
                  name="ocrEngine"
                  value={engine.id}
                  checked={active}
                  disabled={engine.status === 'unavailable'}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      ocrEngine: e.target.value as OcrEngineId,
                    }));
                    void saveConfig();
                  }}
                  className="mt-2.5"
                />

                <div className="w-9 h-9 rounded-lg bg-bg-tertiary border border-border flex items-center justify-center shrink-0 text-text-secondary">
                  <Icon size={18} strokeWidth={1.75} />
                </div>

                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-0.5">
                    <span className="text-sm font-medium tracking-tight text-text-primary">
                      {t(engine.nameKey)}
                    </span>
                    {engine.recommended && !active && (
                      <Badge variant="success">{t('settings.ocr.badgeRec')}</Badge>
                    )}
                  </div>
                  <p className="ui-caption mb-2 leading-relaxed">{t(engine.descKey)}</p>
                  <div className="flex flex-wrap gap-1.5">
                    {engine.badgeKeys.map((badge, idx) => (
                      <Badge key={idx} variant={badge.variant}>
                        {t(badge.labelKey)}
                      </Badge>
                    ))}
                  </div>
                </div>
              </label>
            );
          })}
        </div>
      </Card>

      {(currentEngine === 'rapid' || currentEngine === 'paddle') && (
        <Card title={t('settings.ocr.offlineTitle')} description={t('settings.ocr.offlineDesc')}>
          <div className="space-y-3">
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.ocr.pluginDir')}
              </label>
              <input
                type="text"
                value={config.offlineOcr?.pluginDir || ''}
                onChange={(e) => {
                  updateConfig((prev) => ({
                    ...prev,
                    offlineOcr: {
                      backend: currentEngine === 'paddle' ? 'paddle' : 'rapid',
                      pluginDir: e.target.value,
                    },
                  }));
                }}
                onBlur={() => void saveConfig()}
                placeholder={t('settings.ocr.pluginDirPh')}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
              <p className="ui-caption mt-1.5">{t('settings.ocr.pluginDirHint')}</p>
            </div>
          </div>
        </Card>
      )}

      <Card title={t('settings.ocr.pdfTitle')} description={t('settings.ocr.pdfDesc')}>
        <div className="space-y-3">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.ocr.pdfEngine')}
            </label>
            <select
              value={config.pdfExtractionEngine || 'pdf-extract'}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  pdfExtractionEngine: e.target.value,
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            >
              <option value="pdf-extract">{t('settings.ocr.pdfExtract')}</option>
              <option value="ocr">{t('settings.ocr.pdfOcr')}</option>
              <option value="mineru">{t('settings.ocr.pdfMineru')}</option>
              <option value="marker">{t('settings.ocr.pdfMarker')}</option>
              <option value="ocrmypdf">{t('settings.ocr.pdfOcrmypdf')}</option>
            </select>
            <p className="ui-caption mt-1.5 leading-relaxed">{t('settings.ocr.pdfHint')}</p>
          </div>
          {(config.pdfExtractionEngine === 'mineru' ||
            config.pdfExtractionEngine === 'marker' ||
            config.pdfExtractionEngine === 'ocrmypdf') && (
            <div className="space-y-2">
              <label className="block text-sm font-medium text-text-primary">
                {t('settings.ocr.sidecarPath')}
              </label>
              {config.pdfExtractionEngine === 'mineru' && (
                <input
                  type="text"
                  value={config.pdfExtractionSidecar?.mineruCmd || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      pdfExtractionSidecar: {
                        ...prev.pdfExtractionSidecar,
                        mineruCmd: e.target.value,
                      },
                    }));
                  }}
                  onBlur={() => void saveConfig()}
                  placeholder="magic-pdf"
                  className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
                />
              )}
              {config.pdfExtractionEngine === 'marker' && (
                <input
                  type="text"
                  value={config.pdfExtractionSidecar?.markerCmd || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      pdfExtractionSidecar: {
                        ...prev.pdfExtractionSidecar,
                        markerCmd: e.target.value,
                      },
                    }));
                  }}
                  onBlur={() => void saveConfig()}
                  placeholder="marker_single"
                  className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
                />
              )}
              {config.pdfExtractionEngine === 'ocrmypdf' && (
                <input
                  type="text"
                  value={config.pdfExtractionSidecar?.ocrmypdfCmd || ''}
                  onChange={(e) => {
                    updateConfig((prev) => ({
                      ...prev,
                      pdfExtractionSidecar: {
                        ...prev.pdfExtractionSidecar,
                        ocrmypdfCmd: e.target.value,
                      },
                    }));
                  }}
                  onBlur={() => void saveConfig()}
                  placeholder="ocrmypdf"
                  className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
                />
              )}
            </div>
          )}
        </div>
      </Card>

      <Card
        title={t('settings.ocr.layoutTitle')}
        description={t('settings.ocr.layoutDesc')}
      >
        <div className="space-y-4">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={config.layoutDetectionEnabled || false}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  layoutDetectionEnabled: e.target.checked,
                }));
                void saveConfig();
              }}
              className="rounded"
            />
            <div>
              <p className="text-sm font-medium text-text-primary">
                {t('settings.ocr.layoutEnable')}
              </p>
              <p className="ui-caption">{t('settings.ocr.layoutEnableHint')}</p>
            </div>
          </label>

          {config.layoutDetectionEnabled && (
            <div className="space-y-3 pl-8">
              {/* Model status */}
              <div className="flex items-center gap-2">
                {modelReady ? (
                  <>
                    <CheckCircle size={16} className="text-success" />
                    <span className="text-sm text-text-primary">
                      {t('settings.ocr.layoutModelReady')}
                    </span>
                    {modelSize > 0 && (
                      <Badge variant="success">
                        {(modelSize / 1024 / 1024).toFixed(1)} MB
                      </Badge>
                    )}
                  </>
                ) : (
                  <span className="text-sm text-text-secondary">
                    {t('settings.ocr.layoutModelNotReady')}
                  </span>
                )}
              </div>

              {/* Download / remove buttons */}
              <div className="flex gap-2">
                {!modelReady && !downloading && (
                  <button
                    type="button"
                    onClick={() => void handleDownloadModel()}
                    className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary"
                  >
                    {t('settings.ocr.layoutDownload')}
                  </button>
                )}
                {downloading && (
                  <div className="flex items-center gap-2">
                    <div className="w-48 h-2 bg-bg-tertiary rounded-full overflow-hidden">
                      <div
                        className="h-full bg-primary transition-all"
                        style={{ width: `${downloadPct}%` }}
                      />
                    </div>
                    <span className="text-xs text-text-secondary">{downloadPct}%</span>
                  </div>
                )}
                {modelReady && !downloading && (
                  <button
                    type="button"
                    onClick={() => void handleRemoveModel()}
                    className="px-3 py-2 text-sm rounded-lg border border-border bg-bg-secondary hover:bg-bg-tertiary"
                  >
                    {t('settings.ocr.layoutRemove')}
                  </button>
                )}
              </div>

              {downloadError && (
                <p className="text-xs text-error">{downloadError}</p>
              )}

              <p className="ui-caption">
                {t('settings.ocr.layoutNote')}
              </p>
            </div>
          )}
        </div>
      </Card>

      <Card title={t('settings.ocr.paramsTitle')} description={t('settings.ocr.paramsDesc')}>
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.ocr.interval')}
            </label>
            <input
              type="number"
              min={OCR_WATCH_INTERVAL_MIN_MS}
              max={10000}
              step={100}
              value={config.ocrInterval ?? OCR_WATCH_INTERVAL_DEFAULT_MS}
              onChange={(e) => {
                const n = parseInt(e.target.value, 10);
                updateConfig((prev) => ({
                  ...prev,
                  ocrInterval: Number.isFinite(n)
                    ? Math.min(10000, Math.max(OCR_WATCH_INTERVAL_MIN_MS, n))
                    : OCR_WATCH_INTERVAL_DEFAULT_MS,
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg"
            />
            <p className="ui-caption mt-1.5 leading-relaxed">
              {t('settings.ocr.intervalHint', {
                default: OCR_WATCH_INTERVAL_DEFAULT_MS,
                min: OCR_WATCH_INTERVAL_MIN_MS,
              })}
            </p>
          </div>
        </div>
      </Card>
    </div>
  );
}
