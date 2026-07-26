// ─── OCR Window Binding ──────────────────────────────────────────────────────
// Manages binding the OCR region to a target window and tracking its movement.

import { safeInvoke } from '../services/invoke';
import type { OcrRegion } from '../types/ocr';

export interface BoundWindow {
  hwnd: number;
  title: string;
  /** Region offset relative to window top-left at bind time */
  offset: { dx: number; dy: number; width: number; height: number };
  /** Last known window rect */
  lastRect: { x: number; y: number; width: number; height: number } | null;
}

interface WindowRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Poll target window rect. 50ms ≈ 20fps — much snappier than 500ms. */
export const FOLLOW_POLL_MS = 50;

export interface WindowBindingCallbacks {
  onRegionUpdate: (region: OcrRegion) => void;
  onWindowMinimized: () => void;
  onWindowRestored: () => void;
  onOverlayPositionSync: (x: number, y: number) => void;
}

export class WindowBindingManager {
  private hwnd = 0;
  private followTimer: ReturnType<typeof setInterval> | null = null;
  private boundWindow: BoundWindow | null = null;
  private regionRef: OcrRegion | null = null;
  private overlayCreatedRef = false;
  private callbacks: WindowBindingCallbacks;
  private wasMinimized = false;
  /** Serialize follow polls so stale get_window_rect cannot reverse moves. */
  private pollGen = 0;
  private pollInFlight = false;

  constructor(callbacks: WindowBindingCallbacks) {
    this.callbacks = callbacks;
  }

  /** Current bound window info */
  getBoundWindow(): BoundWindow | null {
    return this.boundWindow;
  }

  /** Current target HWND */
  getHwnd(): number {
    return this.hwnd;
  }

  /** Mark that overlay has been created (for position sync) */
  setOverlayCreated(created: boolean): void {
    this.overlayCreatedRef = created;
  }

  /** Update the tracked region reference (used by follow loop) */
  setRegionRef(region: OcrRegion | null): void {
    this.regionRef = region;
  }

  private isOwnChromeTitle(title: string): boolean {
    return (
      title === 'OCR Region' ||
      title === 'OCR Screenshot' ||
      title === 'OCR-v2 Screenshot' ||
      title === 'Moon Translator'
    );
  }

  /** Try hwnd_from_point at several spots inside the capture rect (I6). */
  private async hwndFromRegion(region: OcrRegion): Promise<number | null> {
    const pts = [
      { x: region.x + region.width / 2, y: region.y + region.height / 2 },
      { x: region.x + region.width * 0.25, y: region.y + region.height * 0.35 },
      { x: region.x + region.width * 0.75, y: region.y + region.height * 0.35 },
      {
        x: region.x + region.width / 2,
        y: region.y + Math.min(region.height * 0.7, region.height - 4),
      },
    ];
    for (const p of pts) {
      const [hwnd, err] = await safeInvoke<number>(
        'hwnd_from_point',
        { x: Math.round(p.x), y: Math.round(p.y) },
        { silent: true },
      );
      if (err || !hwnd || hwnd <= 0) continue;
      const [title] = await safeInvoke<string>('get_window_title_cmd', { hwnd }, { silent: true });
      if (this.isOwnChromeTitle(title || '')) continue;
      return hwnd;
    }
    return null;
  }

  /** Bind to the window under the OCR region (fallback: foreground) and start tracking */
  async bind(region: OcrRegion): Promise<BoundWindow | null> {
    let hwnd = await this.hwndFromRegion(region);
    if (!hwnd) {
      const [fg, fgErr] = await safeInvoke<number>('detect_foreground_hwnd', undefined, {
        silent: true,
      });
      if (fgErr || !fg || fg <= 0) return null;
      const [title] = await safeInvoke<string>(
        'get_window_title_cmd',
        { hwnd: fg },
        { silent: true },
      );
      if (this.isOwnChromeTitle(title || '')) return null;
      hwnd = fg;
    }

    const [rect, rectErr] = await safeInvoke<WindowRect | null>(
      'get_window_rect_cmd',
      { hwnd },
      { silent: true },
    );
    if (rectErr || !rect) return null;

    const [title] = await safeInvoke<string>('get_window_title_cmd', { hwnd }, { silent: true });
    const titleStr = title || '';
    if (this.isOwnChromeTitle(titleStr)) return null;

    const bound: BoundWindow = {
      hwnd,
      title: titleStr || `Window ${hwnd}`,
      offset: {
        dx: region.x - rect.x,
        dy: region.y - rect.y,
        width: region.width,
        height: region.height,
      },
      lastRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
    };

    this.hwnd = hwnd;
    this.boundWindow = bound;
    this.startFollowLoop(hwnd);
    return bound;
  }

  /** Stop tracking and clear binding state */
  unbind(): void {
    if (this.followTimer) {
      clearInterval(this.followTimer);
      this.followTimer = null;
    }
    this.hwnd = 0;
    this.boundWindow = null;
    this.wasMinimized = false;
  }

  /** Unbind then rebind to the current foreground window */
  async rebind(region: OcrRegion): Promise<BoundWindow | null> {
    this.unbind();
    return this.bind(region);
  }

  /**
   * Keep the same HWND, only refresh the region offset (after user drags/resizes
   * the OCR frame). Avoids re-detecting foreground which would bind to the OCR
   * frame itself.
   */
  async refreshOffset(region: OcrRegion): Promise<boolean> {
    if (!this.boundWindow || this.hwnd <= 0) return false;

    const [rect, rectErr] = await safeInvoke<WindowRect | null>(
      'get_window_rect_cmd',
      { hwnd: this.hwnd },
      { silent: true },
    );
    if (rectErr || !rect || (rect.width === 0 && rect.height === 0)) return false;

    this.regionRef = region;
    this.boundWindow = {
      ...this.boundWindow,
      offset: {
        dx: region.x - rect.x,
        dy: region.y - rect.y,
        width: region.width,
        height: region.height,
      },
      lastRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
    };
    return true;
  }

  /** Clean up timers */
  dispose(): void {
    if (this.followTimer) {
      clearInterval(this.followTimer);
      this.followTimer = null;
    }
  }

  private startFollowLoop(hwnd: number): void {
    if (this.followTimer) {
      clearInterval(this.followTimer);
    }

    this.followTimer = setInterval(() => {
      if (this.pollInFlight) return;
      this.pollInFlight = true;
      const gen = ++this.pollGen;
      void (async () => {
        try {
          const [rect] = await safeInvoke<WindowRect | null>(
            'get_window_rect_cmd',
            { hwnd },
            { silent: true },
          );
          if (gen !== this.pollGen) return;
          if (!rect || !this.regionRef || !this.boundWindow) return;

          const bw = this.boundWindow;
          const lastRect = bw.lastRect;

          if (rect.width === 0 && rect.height === 0) {
            if (!this.wasMinimized) {
              this.wasMinimized = true;
              this.callbacks.onWindowMinimized();
            }
            return;
          }

          const shouldMove =
            !lastRect ||
            Math.abs(rect.x - lastRect.x) >= 1 ||
            Math.abs(rect.y - lastRect.y) >= 1 ||
            Math.abs(rect.width - lastRect.width) >= 1 ||
            Math.abs(rect.height - lastRect.height) >= 1;

          if (shouldMove) {
            const newRegion: OcrRegion = {
              x: rect.x + bw.offset.dx,
              y: rect.y + bw.offset.dy,
              width: bw.offset.width,
              height: bw.offset.height,
            };
            this.regionRef = newRegion;
            this.boundWindow = {
              ...bw,
              lastRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
            };
            this.callbacks.onRegionUpdate(newRegion);

            if (this.overlayCreatedRef) {
              this.callbacks.onOverlayPositionSync(newRegion.x + newRegion.width + 10, newRegion.y);
            }
          }

          if (this.wasMinimized && rect.width > 0 && rect.height > 0) {
            this.wasMinimized = false;
            this.callbacks.onWindowRestored();
          }
        } finally {
          if (gen === this.pollGen) this.pollInFlight = false;
        }
      })();
    }, FOLLOW_POLL_MS);
  }
}
