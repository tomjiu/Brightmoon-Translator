// 轻量语言检测 - 用于学习场景 TTS 发音语言选择

const KANA_REGEX = /[\u3040-\u30ff]/;
const CJK_REGEX = /[\u4e00-\u9fff]/;
const HANGUL_REGEX = /[\uac00-\ud7af]/;
const ASCII_LETTER_REGEX = /^[a-zA-Z]/;

/**
 * 根据文本内容启发式判断 TTS 发音语言。
 * 优先级：日文（含假名）> 韩文（谚文）> 中文（含汉字）> 英文（拉丁字母）> 默认英文。
 */
export function detectSpeakLang(text: string): string {
  const trimmed = text.trim();
  if (KANA_REGEX.test(trimmed)) {
    return 'ja';
  }
  if (HANGUL_REGEX.test(trimmed)) {
    return 'ko';
  }
  if (CJK_REGEX.test(trimmed)) {
    return 'zh';
  }
  if (ASCII_LETTER_REGEX.test(trimmed)) {
    return 'en';
  }
  return 'en';
}
