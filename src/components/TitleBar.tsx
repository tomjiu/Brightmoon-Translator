import { useCallback, useEffect, useState } from 'react';
import { Minus, Square, Copy, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '../services/tauriRuntime';

/**
 * Frameless window chrome (decorations:false). Always paint buttons —
 * never gate on a one-shot isTauri check (that raced IPC inject → empty chrome).
 */
export default function TitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    const win = getCurrentWindow();
    void win
      .isMaximized()
      .then(setMaximized)
      .catch(() => undefined);
    void win
      .onResized(() => {
        void win
          .isMaximized()
          .then(setMaximized)
          .catch(() => undefined);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => unlisten?.();
  }, []);

  const minimize = useCallback(async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (e) {
      console.warn('[TitleBar] minimize failed', e);
    }
  }, []);

  const toggleMax = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      await win.toggleMaximize();
      setMaximized(await win.isMaximized());
    } catch (e) {
      console.warn('[TitleBar] toggleMaximize failed', e);
    }
  }, []);

  const close = useCallback(async () => {
    try {
      await getCurrentWindow().close();
    } catch (e) {
      console.warn('[TitleBar] close failed', e);
    }
  }, []);

  return (
    <div
      className="h-9 shrink-0 flex items-stretch border-b border-border select-none z-50 relative"
      style={{ background: 'var(--color-bg-chrome, #0a0a0a)' }}
      data-titlebar="1"
    >
      <div
        className="flex-1 min-w-0"
        data-tauri-drag-region
        onDoubleClick={() => void toggleMax()}
      />
      <div className="flex items-stretch shrink-0" data-window-controls="1">
        <button
          type="button"
          onClick={() => void minimize()}
          className="w-11 flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
          aria-label="minimize"
          title="最小化"
        >
          <Minus size={14} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          onClick={() => void toggleMax()}
          className="w-11 flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
          aria-label={maximized ? 'restore' : 'maximize'}
          title={maximized ? '还原' : '最大化'}
        >
          {maximized ? (
            <Copy size={12} strokeWidth={1.75} />
          ) : (
            <Square size={12} strokeWidth={1.75} />
          )}
        </button>
        <button
          type="button"
          onClick={() => void close()}
          className="w-11 flex items-center justify-center text-text-secondary hover:bg-primary hover:text-primary-fg transition-colors"
          aria-label="close"
          title="关闭"
        >
          <X size={14} strokeWidth={1.75} />
        </button>
      </div>
    </div>
  );
}
