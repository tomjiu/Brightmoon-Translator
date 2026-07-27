import { useCallback, useEffect, useState } from 'react';
import { Minus, Square, Copy, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime } from '../services/tauriRuntime';

/**
 * Frameless window chrome: no product title text, theme-colored strip.
 * Drag via data-tauri-drag-region; controls on the right (Windows layout).
 */
export default function TitleBar() {
  const isTauri = isTauriRuntime();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    const win = getCurrentWindow();
    void win.isMaximized().then(setMaximized);
    void win
      .onResized(() => {
        void win.isMaximized().then(setMaximized);
      })
      .then((fn) => {
        unlisten = fn;
      });
    return () => unlisten?.();
  }, [isTauri]);

  const minimize = useCallback(async () => {
    if (!isTauri) return;
    await getCurrentWindow().minimize();
  }, [isTauri]);

  const toggleMax = useCallback(async () => {
    if (!isTauri) return;
    const win = getCurrentWindow();
    await win.toggleMaximize();
    setMaximized(await win.isMaximized());
  }, [isTauri]);

  const close = useCallback(async () => {
    if (!isTauri) return;
    await getCurrentWindow().close();
  }, [isTauri]);

  if (!isTauri) return null;

  return (
    <div className="h-9 shrink-0 flex items-stretch bg-bg-chrome border-b border-border select-none">
      <div
        className="flex-1 min-w-0"
        data-tauri-drag-region
        onDoubleClick={() => void toggleMax()}
      />
      <div className="flex items-stretch shrink-0">
        <button
          type="button"
          onClick={() => void minimize()}
          className="w-11 flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
          aria-label="minimize"
        >
          <Minus size={14} strokeWidth={1.75} />
        </button>
        <button
          type="button"
          onClick={() => void toggleMax()}
          className="w-11 flex items-center justify-center text-text-secondary hover:bg-bg-tertiary hover:text-text-primary transition-colors"
          aria-label={maximized ? 'restore' : 'maximize'}
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
        >
          <X size={14} strokeWidth={1.75} />
        </button>
      </div>
    </div>
  );
}
