import { describe, expect, it } from 'vitest';
import {
  DEFAULT_ENGINE_ORDER,
  ENGINE_META,
  ENGINE_SECTIONS,
  enginesInSection,
  getEngineSection,
  isLlmConfigured,
} from './enginesMeta';

describe('enginesMeta', () => {
  it('lists every default order id in ENGINE_META', () => {
    const ids = new Set(ENGINE_META.map((m) => m.id));
    for (const id of DEFAULT_ENGINE_ORDER) {
      expect(ids.has(id)).toBe(true);
    }
  });

  it('assigns every engine a known section', () => {
    const sectionIds = new Set(ENGINE_SECTIONS.map((s) => s.id));
    for (const meta of ENGINE_META) {
      expect(sectionIds.has(meta.section)).toBe(true);
    }
  });

  it('groups official / web / offline / llm without dropping ids', () => {
    expect(getEngineSection('llm')).toBe('llm');
    expect(getEngineSection('google')).toBe('official');
    expect(getEngineSection('baidu')).toBe('official');
    expect(getEngineSection('youdao')).toBe('web');
    expect(getEngineSection('baidu_web')).toBe('web');
    expect(getEngineSection('offline')).toBe('offline');

    const all = enginesInSection(DEFAULT_ENGINE_ORDER, 'llm')
      .concat(enginesInSection(DEFAULT_ENGINE_ORDER, 'official'))
      .concat(enginesInSection(DEFAULT_ENGINE_ORDER, 'web'))
      .concat(enginesInSection(DEFAULT_ENGINE_ORDER, 'offline'));
    expect(new Set(all).size).toBe(DEFAULT_ENGINE_ORDER.length);
  });

  it('isLlmConfigured requires non-empty key', () => {
    expect(isLlmConfigured({ apiKey: '' })).toBe(false);
    expect(isLlmConfigured({ apiKey: 'k' })).toBe(true);
    expect(isLlmConfigured({ apiKeys: ['', 'x'] })).toBe(true);
  });
});
