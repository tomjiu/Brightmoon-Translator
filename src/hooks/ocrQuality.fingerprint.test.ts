import { describe, expect, it } from 'vitest';
import { imageFingerprint } from './ocrQuality';

describe('imageFingerprint', () => {
  it('is stable for identical payloads', () => {
    const a = `data:image/png;base64,AAAA${'BB'.repeat(200)}`;
    expect(imageFingerprint(a)).toBe(imageFingerprint(a));
  });

  it('differs when content length or body changes', () => {
    const a = `data:image/png;base64,${'A'.repeat(400)}`;
    const b = `data:image/png;base64,${'B'.repeat(400)}`;
    const c = `data:image/png;base64,${'A'.repeat(500)}`;
    expect(imageFingerprint(a)).not.toBe(imageFingerprint(b));
    expect(imageFingerprint(a)).not.toBe(imageFingerprint(c));
  });

  it('handles raw base64 without data-url prefix', () => {
    const raw = 'XYZ'.repeat(100);
    expect(imageFingerprint(raw)).toMatch(/^\d+:[0-9a-f]+$/);
  });
});
