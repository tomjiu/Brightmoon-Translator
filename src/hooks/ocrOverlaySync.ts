// ─── OCR Overlay Sync ────────────────────────────────────────────────────────
// Manages overlay window creation, content updates, and position syncing.

import { invoke } from "@tauri-apps/api/core";
import type { OcrRegion } from "./useOcrMonitor";

export class OverlaySyncManager {
  private created = false;
  private lastText = "";

  /** Whether the overlay has been created */
  isCreated(): boolean {
    return this.created;
  }

  /** Get the last rendered text */
  getLastText(): string {
    return this.lastText;
  }

  /**
   * Create or update the overlay with translated text.
   * Uses incremental updates: creates on first call, then updates content only.
   */
  async update(region: OcrRegion, translatedText: string): Promise<void> {
    const textChanged = translatedText !== this.lastText;

    if (!this.created) {
      // First creation: create overlay with position and text
      const overlayX = region.x + region.width + 10;
      const overlayY = region.y;
      await invoke("update_overlay", {
        x: overlayX,
        y: overlayY,
        width: 350,
        height: 200,
        text: translatedText,
        showControls: true,
      });
      this.created = true;
      this.lastText = translatedText;
    } else if (textChanged) {
      // Text changed but overlay exists: update content only (preserves pin/click-through)
      await invoke("update_overlay_content", {
        source: "",
        translated: translatedText,
      });
      this.lastText = translatedText;
    }
  }

  /** Update overlay position (used when bound window moves) */
  async updatePosition(x: number, y: number): Promise<void> {
    if (!this.created) return;
    try {
      await invoke("update_overlay_position", { x, y });
    } catch {
      // Overlay may have been closed externally
    }
  }

  /** Reset state (when monitoring stops) */
  reset(): void {
    this.created = false;
    this.lastText = "";
  }
}
