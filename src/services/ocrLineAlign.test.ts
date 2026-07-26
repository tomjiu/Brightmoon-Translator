import { describe, expect, it } from 'vitest';
import { alignTranslationToLines, tokenizeForAlign } from './ocrLineAlign';

describe('ocrLineAlign', () => {
  it('1:1 when newline counts match', () => {
    expect(alignTranslationToLines(['Hello', 'World'], '你好\n世界')).toEqual(['你好', '世界']);
  });

  it('single source line takes full translation', () => {
    expect(alignTranslationToLines(['Hello world'], '你好世界')).toEqual(['你好世界']);
  });

  it('packs single blob across multiple lines without empty tails', () => {
    const src = ['Short', 'A bit longer line here'];
    const out = alignTranslationToLines(src, '短 这里是更长的一行译文内容');
    expect(out).toHaveLength(2);
    expect(out.join('')).toContain('短');
    expect(out.some((s) => s.length > 0)).toBe(true);
  });

  it('tokenizes CJK as single units', () => {
    expect(tokenizeForAlign('ab你好')).toEqual(['ab', '你', '好']);
  });

  it('drops blank lines then matches count', () => {
    expect(alignTranslationToLines(['A', 'B'], '甲\n\n乙\n')).toEqual(['甲', '乙']);
  });
});
