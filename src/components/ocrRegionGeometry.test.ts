import { describe, expect, it } from 'vitest';
import { captureToFrameRegion, frameToCaptureRegion } from './ocrRegionGeometry';

describe('ocrRegionGeometry', () => {
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
});
