export interface RegionRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Single source for OCR frame geometry (CSS logical px) on the FE.
 * Rust source of truth: `src-tauri/src/ocr_region_consts.rs`
 * (OCR_TOOLBAR_CSS_PX / OCR_MIN_FRAME_CSS_W). Keep values identical (I2/I3).
 */
export const OCR_TOOLBAR_HEIGHT_CSS = 32;
/** Min frame width = full toolbar (icons + language). Keep TS+Rust in sync (I3). */
/** Toolbar fits langs + engine select + engine toggle + pin/follow/watch (I3). */
export const OCR_MIN_FRAME_WIDTH_CSS = 460;
/** Expand selection crop slightly so edge glyphs are not clipped (image px, 1:1 physical). */
export const OCR_SELECTION_PAD_PX = 2;

export function frameToCaptureRegion(
  frame: RegionRect,
  toolbarHeightLogical: number,
  scaleFactor: number,
): RegionRect {
  const toolbarPhysical = toolbarHeightLogical * scaleFactor;
  return {
    x: Math.round(frame.x),
    y: Math.round(frame.y + toolbarPhysical),
    width: Math.round(frame.width),
    height: Math.max(1, Math.round(frame.height - toolbarPhysical)),
  };
}

export function captureToFrameRegion(
  capture: RegionRect,
  toolbarHeightLogical: number,
  scaleFactor: number,
): RegionRect {
  const toolbarPhysical = toolbarHeightLogical * scaleFactor;
  return {
    x: Math.round(capture.x),
    y: Math.round(capture.y - toolbarPhysical),
    width: Math.round(capture.width),
    height: Math.round(capture.height + toolbarPhysical),
  };
}

/** CSS size for painting a contain/left-top image (used by region frame background). */
export function containImageCssSize(
  contentW: number,
  contentH: number,
  imageW: number,
  imageH: number,
): { width: number; height: number } {
  if (imageW <= 0 || imageH <= 0 || contentW <= 0 || contentH <= 0) {
    return { width: Math.max(0, contentW), height: Math.max(0, contentH) };
  }
  const s = Math.min(contentW / imageW, contentH / imageH);
  return { width: imageW * s, height: imageH * s };
}

/**
 * Where the captured image is painted inside the content box.
 * Contain + horizontal center (vertical top):
 * when the window is min-width-expanded wider than the capture, extra space is
 * split left/right so chrome is not only larger on the right. Rust create/move
 * also expands min-width symmetrically. OCR boxes use the same rect origin.
 */
export function fitImageDisplayRect(
  contentCssWidth: number,
  contentCssHeight: number,
  imagePixelWidth: number,
  imagePixelHeight: number,
): RegionRect {
  if (
    imagePixelWidth <= 0 ||
    imagePixelHeight <= 0 ||
    contentCssWidth <= 0 ||
    contentCssHeight <= 0
  ) {
    return {
      x: 0,
      y: 0,
      width: Math.max(0, contentCssWidth),
      height: Math.max(0, contentCssHeight),
    };
  }
  const scale = Math.min(contentCssWidth / imagePixelWidth, contentCssHeight / imagePixelHeight);
  const width = imagePixelWidth * scale;
  const height = imagePixelHeight * scale;
  return {
    x: Math.max(0, (contentCssWidth - width) / 2),
    y: 0,
    width,
    height,
  };
}

/**
 * Map OCR line boxes (image pixels) into content CSS pixels using the actual
 * painted image rect (contain / left-top), not a full-bleed stretch.
 */
/** Natural size of a data-URL image (for I5 before frame img onLoad). */
export function probeDataUrlImageSize(dataUrl: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve) => {
    if (!dataUrl) {
      resolve({ width: 0, height: 0 });
      return;
    }
    const img = new Image();
    img.onload = () => resolve({ width: img.naturalWidth || 0, height: img.naturalHeight || 0 });
    img.onerror = () => resolve({ width: 0, height: 0 });
    img.src = dataUrl;
  });
}

export function ocrLineToCssRect(
  line: { x: number; y: number; width: number; height: number },
  contentCssWidth: number,
  contentCssHeight: number,
  imagePixelWidth: number,
  imagePixelHeight: number,
  fallbackScale = 1,
): RegionRect {
  const display =
    imagePixelWidth > 0 && imagePixelHeight > 0 && contentCssWidth > 0 && contentCssHeight > 0
      ? fitImageDisplayRect(contentCssWidth, contentCssHeight, imagePixelWidth, imagePixelHeight)
      : {
          x: 0,
          y: 0,
          width: contentCssWidth,
          height: contentCssHeight,
        };

  const sx =
    imagePixelWidth > 0 && display.width > 0
      ? display.width / imagePixelWidth
      : 1 / Math.max(fallbackScale, 0.01);
  const sy =
    imagePixelHeight > 0 && display.height > 0
      ? display.height / imagePixelHeight
      : 1 / Math.max(fallbackScale, 0.01);

  return {
    x: display.x + line.x * sx,
    y: display.y + line.y * sy,
    width: line.width * sx,
    height: line.height * sy,
  };
}
