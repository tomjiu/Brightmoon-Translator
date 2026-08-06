import { useEffect } from 'react';
import { emit } from '@tauri-apps/api/event';

const READY_EVENT = 'selection-pop-ready';

/**
 * Floating 划词 pop button (Rust `selection-pop` window): a 32×32 "译" chip.
 *
 * Rendered through the React bundle as an App-URL window (same proven path as
 * the translate card), replacing the legacy `data:text/html` webview whose
 * content sometimes failed to paint and left a plain black block. The window is
 * preloaded hidden at startup, so the first show reuses an already-painted
 * chip and never flashes a blank rectangle.
 *
 * Clicks are handled by the Rust Win32 mouse hook (pop hwnd/rect) — this
 * component only paints. It self-reports readiness via the `POPREADY`
 * document.title and the `selection-pop-ready` event so Rust can (a) wait
 * before the first show and (b) log if the webview ever fails to render again.
 */
export default function SelectionPop() {
  useEffect(() => {
    document.title = 'POPREADY';
    void emit(READY_EVENT).catch(() => undefined);
  }, []);

  return (
    <div
      className="w-full h-full flex items-center justify-center select-none cursor-pointer"
      style={{
        borderRadius: 8,
        background: 'var(--color-bg-tertiary)',
        color: 'var(--color-text-primary)',
        border: '1px solid var(--color-border-strong)',
        fontSize: 13,
        fontWeight: 600,
        lineHeight: 1,
      }}
    >
      译
    </div>
  );
}
