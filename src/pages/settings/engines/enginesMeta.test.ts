import { describe, expect, it } from 'vitest';
import { DEFAULT_ENGINE_ORDER, ENGINE_META, isLlmConfigured } from './enginesMeta';

describe('enginesMeta', () => {
  it('lists every default order id in ENGINE_META', () => {
    const ids = new Set(ENGINE_META.map((m) => m.id));
    for (const id of DEFAULT_ENGINE_ORDER) {
      expect(ids.has(id)).toBe(true);
    }
  });

  it('isLlmConfigured requires non-empty key', () => {
    expect(isLlmConfigured({ apiKey: '' })).toBe(false);
    expect(isLlmConfigured({ apiKey: 'k' })).toBe(true);
    expect(isLlmConfigured({ apiKeys: ['', 'x'] })).toBe(true);
  });
});
