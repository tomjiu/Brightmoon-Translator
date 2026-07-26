import { describe, expect, it } from 'vitest';
import {
  OCR_TOOLBAR_HEIGHT_CSS,
  OCR_MIN_FRAME_WIDTH_CSS,
  OCR_SELECTION_PAD_PX,
  captureToFrameRegion,
  containImageCssSize,
  fitImageDisplayRect,
  frameToCaptureRegion,
  ocrLineToCssRect,
} from './ocrRegionGeometry';

describe('ocrRegionGeometry', () => {
  it('exports stable geometry constants for I2/I3', () => {
    expect(OCR_TOOLBAR_HEIGHT_CSS).toBe(32);
    expect(OCR_MIN_FRAME_WIDTH_CSS).toBe(380);
    expect(OCR_SELECTION_PAD_PX).toBe(2);
  });

  it('containImageCssSize matches fitImageDisplayRect dimensions', () => {
    const a = containImageCssSize(300, 100, 400, 200);
    const b = fitImageDisplayRect(300, 100, 400, 200);
    expect(a.width).toBeCloseTo(b.width, 5);
    expect(a.height).toBeCloseTo(b.height, 5);
  });

  it('maps lines with explicit image size without waiting for layout', () => {
    // Payload natural size → contain scale; unknown size falls back to 1/DPR (wrong if only that).
    const line = { x: 10, y: 20, width: 100, height: 16 };
    const withSize = ocrLineToCssRect(line, 200, 100, 400, 200, 1.25);
    expect(withSize.x).toBeCloseTo(5, 5); // 10 * (200/400)
    expect(withSize.y).toBeCloseTo(10, 5);
    expect(withSize.width).toBeCloseTo(50, 5);
    const dprOnly = ocrLineToCssRect(line, 0, 0, 0, 0, 1.25);
    expect(dprOnly.x).toBeCloseTo(10 / 1.25, 5);
    expect(dprOnly.x).not.toBeCloseTo(withSize.x, 1);
  });

  it('converts a frame window rect into the underlying capture rect', () => {
    expect(frameToCaptureRegion({ x: 100, y: 168, width: 400, height: 232 }, 32, 1)).toEqual({
      x: 100,
      y: 200,
      width: 400,
      height: 200,
    });
  });

  it('accounts for DPI scale when converting toolbar height', () => {
    expect(frameToCaptureRegion({ x: 150, y: 252, width: 600, height: 348 }, 32, 1.5)).toEqual({
      x: 150,
      y: 300,
      width: 600,
      height: 300,
    });
  });

  it('converts selected capture rect to desired frame window rect', () => {
    expect(captureToFrameRegion({ x: 100, y: 200, width: 400, height: 200 }, 32, 1)).toEqual({
      x: 100,
      y: 168,
      width: 400,
      height: 232,
    });
  });

  it('preserves negative virtual-desktop coordinates', () => {
    expect(frameToCaptureRegion({ x: -1280, y: -48, width: 640, height: 348 }, 32, 1.5)).toEqual({
      x: -1280,
      y: 0,
      width: 640,
      height: 300,
    });
  });

  it('maps OCR line boxes using contain + horizontal center', () => {
    // Image 400x200; content wider 300x100 → scale by height 0.5 → display 200x100,
    // centered: x offset 50; line (40,20) → (50+20, 10).
    expect(
      ocrLineToCssRect(
        { x: 40, y: 20, width: 100, height: 30 },
        300,
        100,
        400,
        200,
        /* wrong DPR should be ignored */ 1.25,
      ),
    ).toEqual({ x: 70, y: 10, width: 50, height: 15 });
  });

  it('falls back to DPR when image size is unknown', () => {
    expect(ocrLineToCssRect({ x: 30, y: 15, width: 60, height: 24 }, 0, 0, 0, 0, 1.5)).toEqual({
      x: 20,
      y: 10,
      width: 40,
      height: 16,
    });
  });

  it('fitImageDisplayRect centers horizontally when content is wider', () => {
    expect(fitImageDisplayRect(300, 100, 400, 200)).toEqual({
      x: 50,
      y: 0,
      width: 200,
      height: 100,
    });
  });

  // Invariant I2: toolbar height used in capture math must stay 32 logical px
  it('keeps 32px toolbar invariant for capture↔frame conversion at 1.25 DPI', () => {
    const TOOLBAR = 32;
    const scale = 1.25;
    const capture = { x: 100, y: 200, width: 400, height: 80 };
    const frame = captureToFrameRegion(capture, TOOLBAR, scale);
    expect(frame.y).toBe(Math.round(200 - TOOLBAR * scale));
    expect(frameToCaptureRegion(frame, TOOLBAR, scale)).toEqual({
      x: capture.x,
      y: capture.y,
      width: capture.width,
      height: capture.height,
    });
  });
});
