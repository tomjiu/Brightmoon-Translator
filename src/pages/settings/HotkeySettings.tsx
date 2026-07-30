import { useCallback, useState } from 'react';
import { Keyboard } from 'lucide-react';
import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import { isTauriRuntime } from '../../services/tauriRuntime';
import { useI18n } from '../../i18n';

type HotkeyKey =
  | 'ocrTranslate'
  | 'showWindow'
  | 'translateSelection'
  | 'replaceTranslate'
  | 'toggleOverlayClickThrough'
  | 'dictionaryLookup';

const FIELD_KEYS: Array<{ key: HotkeyKey; labelKey: string; placeholder: string }> = [
  { key: 'ocrTranslate', labelKey: 'settings.hotkeys.ocrTranslate', placeholder: 'Ctrl+Shift+T' },
  { key: 'showWindow', labelKey: 'settings.hotkeys.showWindow', placeholder: 'Ctrl+T' },
  {
    key: 'translateSelection',
    labelKey: 'settings.hotkeys.translateSelection',
    placeholder: 'Ctrl+Shift+Y',
  },
  {
    key: 'dictionaryLookup',
    labelKey: 'settings.hotkeys.dictionaryLookup',
    placeholder: 'Ctrl+Shift+D',
  },
  {
    key: 'replaceTranslate',
    labelKey: 'settings.hotkeys.replaceTranslate',
    placeholder: 'Ctrl+Shift+R',
  },
  {
    key: 'toggleOverlayClickThrough',
    labelKey: 'settings.hotkeys.toggleOverlayClickThrough',
    placeholder: 'Ctrl+Shift+Escape',
  },
];

function formatHotkey(e: React.KeyboardEvent): string | null {
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return null;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');

  let key = e.key;
  if (key === ' ') key = 'Space';
  else if (key === 'Escape') key = 'Escape';
  else if (key.length === 1) key = key.toUpperCase();
  else if (key.startsWith('Arrow')) key = key.replace('Arrow', '');
  else key = key.charAt(0).toUpperCase() + key.slice(1);

  parts.push(key);
  return parts.join('+');
}

export default function HotkeySettings() {
  const { t } = useI18n();
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const saveConfig = useConfigStore((s) => s.saveConfig);
  const isTauri = isTauriRuntime();
  const [capturing, setCapturing] = useState<HotkeyKey | null>(null);

  const commit = useCallback(
    (key: HotkeyKey, value: string) => {
      updateConfig((prev) => ({
        ...prev,
        hotkeys: { ...prev.hotkeys, [key]: value },
      }));
      void saveConfig();
      setCapturing(null);
    },
    [saveConfig, updateConfig],
  );

  if (!isTauri) {
    return (
      <div className="space-y-5">
        <div>
          <h1 className="ui-page-title">{t('settings.hotkeys.pageTitle')}</h1>
          <p className="ui-page-desc">{t('settings.hotkeys.desktopOnly')}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">{t('settings.hotkeys.pageTitle')}</h1>
        <p className="ui-page-desc">{t('settings.hotkeys.pageDesc')}</p>
      </div>

      <Card title={t('settings.hotkeys.globalTitle')} description={t('settings.hotkeys.globalDesc')}>
        <div className="space-y-3">
          {FIELD_KEYS.map((f) => {
            const value = config.hotkeys[f.key] || f.placeholder;
            const isCap = capturing === f.key;
            return (
              <div
                key={f.key}
                className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 py-2 border-b border-border last:border-0"
              >
                <label className="sm:w-40 shrink-0 text-sm font-medium text-text-primary">
                  {t(f.labelKey)}
                </label>
                <div className="flex-1 relative">
                  <Keyboard
                    size={14}
                    className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary pointer-events-none"
                  />
                  <input
                    type="text"
                    readOnly
                    value={isCap ? '…' : value}
                    placeholder={f.placeholder}
                    onFocus={() => setCapturing(f.key)}
                    onBlur={() => setCapturing(null)}
                    onKeyDown={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      if (e.key === 'Escape') {
                        setCapturing(null);
                        return;
                      }
                      if (e.key === 'Backspace' || e.key === 'Delete') {
                        commit(f.key, '');
                        return;
                      }
                      const next = formatHotkey(e);
                      if (next) commit(f.key, next);
                    }}
                    className={`w-full pl-9 pr-3 py-2.5 bg-bg-tertiary text-text-primary border rounded-lg text-sm font-mono tracking-tight cursor-pointer transition-colors ${
                      isCap
                        ? 'border-primary ring-0 shadow-[var(--ring)]'
                        : 'border-border hover:border-border-strong'
                    }`}
                  />
                </div>
              </div>
            );
          })}
        </div>
      </Card>
    </div>
  );
}
