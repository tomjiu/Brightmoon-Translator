export function normalizeTranslatorInput(text: string, deleteNewlines: boolean): string {
  if (!deleteNewlines) {
    return text;
  }
  return text.replace(/\s*[\r\n]+\s*/g, ' ');
}
