import { safeInvoke } from "./invoke";

export interface OverlayPosition {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Show or update the translation overlay at a specific position.
 * Shared helper for all overlay use cases.
 */
export async function showOverlayAt(
  pos: OverlayPosition,
  text: string,
  source: string,
  overlayLevel: number = 2
): Promise<void> {
  // Overlay update is best-effort, silently ignore errors
  await safeInvoke("update_overlay", {
    x: Math.round(pos.x),
    y: Math.round(pos.y),
    width: Math.round(pos.width),
    height: Math.round(pos.height),
    text,
    source,
    overlayLevel,
  }, { silent: true });
}

/**
 * Calculate overlay position below a text element.
 * Used by HookMonitor for precise text-positioned overlays.
 */
export function positionBelowText(
  textX: number,
  textY: number,
  textW: number,
  textH: number,
  overlayWidth = 500,
  overlayHeight = 180
): OverlayPosition {
  const w = Math.min(overlayWidth, textW + 60);
  const x = textX + (textW - w) / 2;
  return {
    x,
    y: textY + textH + 8,
    width: w,
    height: overlayHeight,
  };
}

/**
 * Calculate overlay position at the bottom of a window.
 * Used as fallback when text position is not available.
 */
export function positionAtWindowBottom(
  winX: number,
  winY: number,
  winW: number,
  winH: number,
  overlayWidth = 500,
  overlayHeight = 180
): OverlayPosition {
  const w = Math.min(overlayWidth, winW - 40);
  const x = winX + (winW - w) / 2;
  return {
    x,
    y: winY + winH - overlayHeight - 20,
    width: w,
    height: overlayHeight,
  };
}
