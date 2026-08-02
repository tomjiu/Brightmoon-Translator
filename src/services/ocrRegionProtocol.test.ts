import { describe, expect, it, vi, beforeEach } from 'vitest';
import { emitTo } from '@tauri-apps/api/event';
import {
  regionLabel,
  regionEventName,
  DEFAULT_REGION_ID,
  REGION_EVENTS_BY_ID,
  emitToRegionId,
  emitToRegion,
  OcrRegionEvents,
  OcrMainEvents,
} from './ocrRegionProtocol';

vi.mock('@tauri-apps/api/event', () => ({
  emitTo: vi.fn().mockResolvedValue(undefined),
}));

describe('ocrRegionProtocol (M3 multi-region routing)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('regionLabel', () => {
    it('default id maps to the bare legacy label', () => {
      expect(regionLabel(DEFAULT_REGION_ID)).toBe('ocr-region-frame');
    });

    it('non-default id maps to the suffixed label', () => {
      expect(regionLabel('3')).toBe('ocr-region-frame-3');
    });
  });

  describe('REGION_EVENTS_BY_ID', () => {
    it('default id keeps legacy un-suffixed event names', () => {
      expect(REGION_EVENTS_BY_ID.ready(DEFAULT_REGION_ID)).toBe(OcrMainEvents.frameReady);
      expect(REGION_EVENTS_BY_ID.text(DEFAULT_REGION_ID)).toBe(OcrRegionEvents.updateData);
      expect(REGION_EVENTS_BY_ID.error(DEFAULT_REGION_ID)).toBe(OcrRegionEvents.error);
      expect(REGION_EVENTS_BY_ID.mode(DEFAULT_REGION_ID)).toBe(OcrRegionEvents.mode);
    });

    it('non-default id uses the -{id} suffixed event names', () => {
      expect(REGION_EVENTS_BY_ID.ready('2')).toBe('ocr-region-ready-2');
      expect(REGION_EVENTS_BY_ID.text('2')).toBe('ocr-region-text-2');
      expect(REGION_EVENTS_BY_ID.error('2')).toBe('ocr-region-error-2');
      expect(REGION_EVENTS_BY_ID.mode('2')).toBe('ocr-region-mode-2');
    });
  });

  describe('regionEventName', () => {
    it('default id keeps the base name', () => {
      expect(regionEventName(OcrRegionEvents.loading, DEFAULT_REGION_ID)).toBe(
        OcrRegionEvents.loading,
      );
    });

    it('non-default id appends the -{id} suffix (frame→main control events)', () => {
      expect(regionEventName(OcrRegionEvents.loading, '2')).toBe('ocr-region-loading-2');
      expect(regionEventName(OcrMainEvents.continuous, '2')).toBe('ocr-region-continuous-2');
    });
  });

  describe('emitToRegionId', () => {
    it('routes to the region-specific window label', async () => {
      await emitToRegionId('7', 'ocr-region-text-7', { text: 'hello' });
      expect(emitTo).toHaveBeenCalledWith('ocr-region-frame-7', 'ocr-region-text-7', {
        text: 'hello',
      });
    });

    it('default id routes to the bare label', async () => {
      await emitToRegionId(DEFAULT_REGION_ID, OcrRegionEvents.updateData, null);
      expect(emitTo).toHaveBeenCalledWith('ocr-region-frame', OcrRegionEvents.updateData, null);
    });
  });

  describe('emitToRegion (legacy shim)', () => {
    it('delegates to the default region label', async () => {
      await emitToRegion(OcrRegionEvents.error, { message: 'x' });
      expect(emitTo).toHaveBeenCalledWith('ocr-region-frame', OcrRegionEvents.error, {
        message: 'x',
      });
    });
  });
});
