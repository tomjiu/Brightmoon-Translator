/** One-shot handoff: DocumentsViewer → format viewer (no multi-tab UX). */
const KEY = 'moon-pending-doc-path';

export function setPendingDocPath(path: string): void {
  try {
    sessionStorage.setItem(KEY, path);
  } catch {
    /* ignore quota */
  }
}

export function takePendingDocPath(): string | null {
  try {
    const p = sessionStorage.getItem(KEY);
    if (p) sessionStorage.removeItem(KEY);
    return p;
  } catch {
    return null;
  }
}
