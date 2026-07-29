import { useCallback, useState } from 'react';
import { Keyboard } from 'lucide-react';
import { useConfigStore } from '../../stores/configStore';
import Card from '../../components/Card';
import { isTauriRuntime } from '../../services/tauriRuntime';

type HotkeyKey =
  | 'ocrTranslate'
  | 'showWindow'
  | 'translateSelection'
  | 'replaceTranslate'
  | 'toggleOverlayClickThrough'
  | 'dictionaryLookup';

const FIELDS: Array<{ key: HotkeyKey; label: string; placeholder: string }> = [
  { key: 'ocrTranslate', label: 'OCR 截图翻译', placeholder: 'Ctrl+Shift+T' },
  { key: 'showWindow', label: '显示主窗口', placeholder: 'Ctrl+T' },
  { key: 'translateSelection', label: '选中文本翻译', placeholder: 'Ctrl+Shift+Y' },
  {
    key: 'dictionaryLookup',
    label: '选中词典查询（可空=关闭）',
    placeholder: 'Ctrl+Shift+D',
  },
  { key: 'replaceTranslate', label: '替换翻译', placeholder: 'Ctrl+Shift+R' },
  {
    key: 'toggleOverlayClickThrough',
    label: '浮层取消穿透',
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
          <h1 className="ui-page-title">快捷键</h1>
          <p className="ui-page-desc">快捷键仅在桌面版可用</p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div>
        <h1 className="ui-page-title">快捷键</h1>
        <p className="ui-page-desc">
          点击输入框后按下组合键即可录制；保存后立即生效。自动划词/浮钮请到「划词翻译」页
        </p>
      </div>

      <Card title="全局快捷键" description="系统级热键 · Lucide 图标导航对应功能">
        <div className="space-y-3">
          {FIELDS.map((f) => {
            const value = config.hotkeys[f.key] || f.placeholder;
            const isCap = capturing === f.key;
            return (
              <div
                key={f.key}
                className="flex flex-col sm:flex-row sm:items-center gap-2 sm:gap-4 py-2 border-b border-border last:border-0"
              >
                <label className="sm:w-40 shrink-0 text-sm font-medium text-text-primary">
                  {f.label}
                </label>
                <div className="flex-1 relative">
                  <Keyboard
                    size={14}
                    className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary pointer-events-none"
                  />
                  <input
                    type="text"
                    readOnly
                    value={isCap ? '按下快捷键…' : value}
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
