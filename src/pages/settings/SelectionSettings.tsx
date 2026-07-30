import { useEffect, useMemo, useState } from 'react';
import { useConfigStore } from '../../stores/configStore';
import type { SelectionTriggerMode, SelectionUxConfig } from '../../types';
import Card from '../../components/Card';
import Switch from '../../components/Switch';
import Badge from '../../components/Badge';
import { safeInvoke } from '../../services/invoke';
import { isTauriRuntime } from '../../services/tauriRuntime';
import { useI18n } from '../../i18n';

const DEFAULT_UX: SelectionUxConfig = {
  triggerMode: 'pop_button',
  hoverDictionary: false,
  hoverDwellMs: 400,
  hoverUnit: 'word',
  hoverDictSource: 'auto',
  ocrForcePickup: false,
  ocrModifierKey: '',
  autoMinChars: 1,
  minDragPx: 10,
  excludeProcesses: [],
};

export default function SelectionSettings() {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const [ecdictLoaded, setEcdictLoaded] = useState<boolean | null>(null);

  const TRIGGERS = useMemo(
    (): Array<{
      id: SelectionTriggerMode;
      label: string;
      description: string;
      recommended?: boolean;
    }> => [
      {
        id: 'pop_button',
        label: t('settings.selection.popButton'),
        description: t('settings.selection.popButtonDesc'),
        recommended: true,
      },
      {
        id: 'auto_on_select',
        label: t('settings.selection.autoOnSelect'),
        description: t('settings.selection.autoOnSelectDesc'),
      },
      {
        id: 'hotkey_only',
        label: t('settings.selection.hotkeyOnly'),
        description: t('settings.selection.hotkeyOnlyDesc'),
      },
    ],
    [t],
  );

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void safeInvoke<{ loaded: boolean }>('ecdict_status', undefined, { silent: true }).then(
      ([status]) => {
        if (status) setEcdictLoaded(status.loaded);
      },
    );
  }, []);

  const ux: SelectionUxConfig = {
    ...DEFAULT_UX,
    ...(config.selectionUx ?? {}),
  };

  const patchUx = (partial: Partial<SelectionUxConfig>) => {
    updateConfig((prev) => ({
      ...prev,
      selectionUx: {
        ...DEFAULT_UX,
        ...(prev.selectionUx ?? {}),
        ...partial,
      },
    }));
    void saveConfig();
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.selection.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.selection.pageDesc')}</p>
      </div>

      <Card
        title={t('settings.selection.triggerTitle')}
        description={t('settings.selection.triggerDesc')}
      >
        <div className="grid gap-2">
          {TRIGGERS.map((item) => (
            <label
              key={item.id}
              className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-colors ${
                ux.triggerMode === item.id
                  ? 'border-primary bg-primary/5'
                  : 'border-border hover:border-border-strong'
              }`}
            >
              <input
                type="radio"
                name="selectionTrigger"
                value={item.id}
                checked={ux.triggerMode === item.id}
                onChange={() => patchUx({ triggerMode: item.id })}
                className="mt-1"
              />
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-text-primary">{item.label}</span>
                  {item.recommended && (
                    <Badge variant="info">{t('settings.selection.defaultBadge')}</Badge>
                  )}
                </div>
                <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
                  {item.description}
                </p>
              </div>
            </label>
          ))}
        </div>
        {(ux.triggerMode === 'auto_on_select' || ux.triggerMode === 'pop_button') && (
          <div className="mt-4 space-y-4">
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.minChars')}
              </label>
              <input
                type="number"
                min={1}
                max={50}
                value={ux.autoMinChars}
                onChange={(e) => {
                  const n = parseInt(e.target.value, 10);
                  patchUx({
                    autoMinChars: Number.isFinite(n) ? Math.min(50, Math.max(1, n)) : 1,
                  });
                }}
                className="w-24 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.minDrag')}
              </label>
              <input
                type="number"
                min={1}
                max={80}
                value={ux.minDragPx ?? 10}
                onChange={(e) => {
                  const n = parseInt(e.target.value, 10);
                  patchUx({
                    minDragPx: Number.isFinite(n) ? Math.min(80, Math.max(1, n)) : 10,
                  });
                }}
                className="w-24 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
              <p className="ui-caption mt-1.5">{t('settings.selection.minDragHint')}</p>
            </div>
          </div>
        )}
        <div className="mt-4">
          <label className="block text-sm font-medium text-text-primary mb-2">
            {t('settings.selection.excludeProc')}
          </label>
          <input
            type="text"
            value={(ux.excludeProcesses ?? []).join(', ')}
            onChange={(e) => {
              const list = e.target.value
                .split(/[,，\s]+/)
                .map((s) => s.trim())
                .filter(Boolean);
              patchUx({ excludeProcesses: list });
            }}
            placeholder={t('settings.selection.excludeProcPh')}
            className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
          />
          <p className="ui-caption mt-1.5">{t('settings.selection.excludeProcHint')}</p>
        </div>
        <p className="ui-caption mt-3 leading-relaxed">{t('settings.selection.hotkeyNote')}</p>
      </Card>

      <Card
        title={t('settings.selection.hoverTitle')}
        description={t('settings.selection.hoverDesc')}
      >
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-sm font-medium text-text-primary">
              {t('settings.selection.hoverEnable')}
            </p>
            <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
              {t('settings.selection.hoverEnableHint')}
            </p>
          </div>
          <Switch checked={ux.hoverDictionary} onChange={(v) => patchUx({ hoverDictionary: v })} />
        </div>
        {ux.hoverDictionary && (
          <div className="mt-4 space-y-3">
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.hoverDwell')}
              </label>
              <input
                type="number"
                min={150}
                max={2000}
                step={50}
                value={ux.hoverDwellMs}
                onChange={(e) => {
                  const n = parseInt(e.target.value, 10);
                  patchUx({
                    hoverDwellMs: Number.isFinite(n) ? Math.min(2000, Math.max(150, n)) : 400,
                  });
                }}
                className="w-28 px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              />
            </div>
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.hoverUnit')}
              </label>
              <select
                value={ux.hoverUnit || 'word'}
                onChange={(e) => patchUx({ hoverUnit: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="word">{t('settings.selection.hoverUnitWord')}</option>
                <option value="sentence">{t('settings.selection.hoverUnitSentence')}</option>
              </select>
            </div>
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.hoverDictSrc')}
              </label>
              <select
                value={ux.hoverDictSource || 'auto'}
                onChange={(e) => patchUx({ hoverDictSource: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="auto">{t('settings.selection.hoverDictAuto')}</option>
                <option value="ecdict">{t('settings.selection.hoverDictEcdict')}</option>
                <option value="youdao">{t('settings.selection.hoverDictYoudao')}</option>
              </select>
              {ecdictLoaded === false && (
                <p className="text-xs text-warning mt-1">{t('settings.selection.ecdictMissing')}</p>
              )}
              {ecdictLoaded === true && (
                <p className="text-xs text-text-secondary mt-1">{t('settings.selection.ecdictOk')}</p>
              )}
              <p className="text-xs text-text-secondary mt-1">{t('settings.selection.hoverTech')}</p>
            </div>
          </div>
        )}
      </Card>

      <Card
        title={t('settings.selection.ocrForceTitle')}
        description={t('settings.selection.ocrForceDesc')}
      >
        <div className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div className="min-w-0">
              <p className="text-sm font-medium text-text-primary">
                {t('settings.selection.ocrForceEnable')}
              </p>
              <p className="text-xs text-text-secondary mt-0.5 leading-relaxed">
                {t('settings.selection.ocrForceHint')}
              </p>
            </div>
            <Switch checked={ux.ocrForcePickup} onChange={(v) => patchUx({ ocrForcePickup: v })} />
          </div>
          {ux.ocrForcePickup && (
            <div>
              <label className="block text-sm font-medium text-text-primary mb-2">
                {t('settings.selection.ocrMod')}
              </label>
              <select
                value={ux.ocrModifierKey || ''}
                onChange={(e) => patchUx({ ocrModifierKey: e.target.value })}
                className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
              >
                <option value="">{t('settings.selection.ocrModNone')}</option>
                <option value="shift">{t('settings.selection.ocrModShift')}</option>
                <option value="ctrl">{t('settings.selection.ocrModCtrl')}</option>
                <option value="alt">{t('settings.selection.ocrModAlt')}</option>
              </select>
              <p className="text-xs text-text-secondary mt-1">{t('settings.selection.ocrModHint')}</p>
            </div>
          )}
        </div>
      </Card>

      <Card
        title={t('settings.selection.overlayTitle')}
        description={t('settings.selection.overlayDesc')}
      >
        <div className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.selection.overlayLevel')}
            </label>
            <select
              value={config.overlayLevel ?? 2}
              onChange={(e) => {
                updateConfig((prev) => ({
                  ...prev,
                  overlayLevel: parseInt(e.target.value, 10) || 2,
                }));
                void saveConfig();
              }}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            >
              <option value={1}>{t('settings.selection.overlayL1')}</option>
              <option value={2}>{t('settings.selection.overlayL2')}</option>
              <option value={3}>{t('settings.selection.overlayL3')}</option>
            </select>
          </div>
          <div>
            <label className="block text-sm font-medium text-text-primary mb-2">
              {t('settings.selection.overlayDismiss')}
            </label>
            <input
              type="number"
              min={0}
              max={30000}
              step={500}
              value={config.overlayAutoDismissMs ?? 3000}
              onChange={(e) => {
                const n = parseInt(e.target.value, 10);
                updateConfig((prev) => ({
                  ...prev,
                  overlayAutoDismissMs: Number.isFinite(n) ? Math.max(0, n) : 3000,
                }));
              }}
              onBlur={() => void saveConfig()}
              className="w-full px-3 py-2 bg-bg-tertiary text-text-primary border border-border rounded-lg text-sm"
            />
          </div>
        </div>
      </Card>
    </div>
  );
}
