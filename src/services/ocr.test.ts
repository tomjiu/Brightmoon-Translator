import { beforeEach, describe, expect, it, vi } from 'vitest';
import { captureScreenshotRegion } from './ocr';
import { invokeOrThrow } from './invoke';

vi.mock('./invoke', () => ({
  invokeOrThrow: vi.fn().mockResolvedValue('data:image/png;base64,test'),
}));

describe('captureScreenshotRegion', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('passes signed virtual-desktop coordinates to the backend', async () => {
    await captureScreenshotRegion({
      left: -1280.4,
      top: -22.6,
      width: 320.2,
      height: 180.7,
    });

    expect(invokeOrThrow).toHaveBeenCalledWith('capture_screenshot_region', {
      left: -1280,
      top: -23,
      width: 320,
      height: 181,
    });
  });
});
