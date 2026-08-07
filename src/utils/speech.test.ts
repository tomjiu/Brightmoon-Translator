import { describe, it, expect } from 'vitest';
import { detectSpeakLang } from './speech';

describe('detectSpeakLang', () => {
  it('detects Japanese kana', () => {
    expect(detectSpeakLang('こんにちは')).toBe('ja');
    expect(detectSpeakLang('カタカナテスト')).toBe('ja');
  });

  it('detects Chinese hanzi', () => {
    expect(detectSpeakLang('你好')).toBe('zh');
    expect(detectSpeakLang('学习英语')).toBe('zh');
  });

  it('detects English (ASCII letters)', () => {
    expect(detectSpeakLang('hello')).toBe('en');
    expect(detectSpeakLang('Hello World')).toBe('en');
  });

  it('defaults to English for non-letter input', () => {
    expect(detectSpeakLang('')).toBe('en');
    expect(detectSpeakLang('   ')).toBe('en');
    expect(detectSpeakLang('12345')).toBe('en');
    expect(detectSpeakLang('...')).toBe('en');
  });

  it('trims leading/trailing whitespace before detection', () => {
    expect(detectSpeakLang('  日本語を勉強します  ')).toBe('ja');
    expect(detectSpeakLang('  中文  ')).toBe('zh');
  });

  it('gives Japanese priority over Chinese (kana > hanzi)', () => {
    // 日文汉字 + 假名混合 → 日文优先
    expect(detectSpeakLang('日本語を勉強します')).toBe('ja');
  });
});
