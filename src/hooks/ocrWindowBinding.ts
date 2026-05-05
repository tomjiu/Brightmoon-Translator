// ─── OCR Window Binding ──────────────────────────────────────────────────────
// Manages binding the OCR region to a target window and tracking its movement.

import { invoke } from "@tauri-apps/api/core";
import type { OcrRegion } from "./useOcrMonitor";

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

export const FOLLOW_POLL_MS = 500;

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

  /** Bind to the foreground window and start tracking */
  async bind(region: OcrRegion): Promise<BoundWindow | null> {
    try {
      const hwnd = await invoke<number>("detect_foreground_hwnd");
      if (hwnd <= 0) return null;

      const rect = await invoke<WindowRect | null>("get_window_rect_cmd", { hwnd });
      if (!rect) return null;

      let title = "";
      try {
        title = await invoke<string>("get_window_title_cmd", { hwnd });
      } catch {
        title = `Window ${hwnd}`;
      }

      const bound: BoundWindow = {
        hwnd,
        title,
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
    } catch (e) {
      console.warn("[OCR] Failed to bind window:", e);
      return null;
    }
  }

  /** Stop tracking and clear binding state */
  unbind(): void {
    if (this.followTimer) {
      clearInterval(this.followTimer);
      this.followTimer = null;
    }
    this.hwnd = 0;
    this.boundWindow = null;
  }

  /** Unbind then rebind to the current foreground window */
  async rebind(region: OcrRegion): Promise<BoundWindow | null> {
    this.unbind();
    return this.bind(region);
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

    this.followTimer = setInterval(async () => {
      try {
        const rect = await invoke<WindowRect | null>("get_window_rect_cmd", { hwnd });
        if (!rect || !this.regionRef || !this.boundWindow) return;

        const bw = this.boundWindow;
        const lastRect = bw.lastRect;

        // Check if minimized (width/height == 0)
        if (rect.width === 0 && rect.height === 0) {
          this.callbacks.onWindowMinimized();
          return;
        }

        // Check if window moved
        const shouldMove =
          !lastRect ||
          Math.abs(rect.x - lastRect.x) > 2 ||
          Math.abs(rect.y - lastRect.y) > 2;

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

          // Sync overlay position
          if (this.overlayCreatedRef) {
            const overlayX = newRegion.x + newRegion.width + 10;
            const overlayY = newRegion.y;
            this.callbacks.onOverlayPositionSync(overlayX, overlayY);
          }
        }

        // Auto-resume if window is visible again
        if (rect.width > 0 && rect.height > 0) {
          this.callbacks.onWindowRestored();
        }
      } catch {
        // Window might be gone
      }
    }, FOLLOW_POLL_MS);
  }
}
