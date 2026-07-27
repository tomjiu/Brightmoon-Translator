import { invoke } from '@tauri-apps/api/core';
import type { CollectionPushReport } from '../types';

export async function saveAndCollect(opts: {
  word: string;
  translation: string;
  note?: string;
  fromLang?: string;
  toLang?: string;
  /** When true (default), also write local wordbook (auto-push if enabled). */
  localWordbook?: boolean;
}): Promise<{ localOk: boolean; report: CollectionPushReport }> {
  const fromLang = opts.fromLang ?? 'en';
  const toLang = opts.toLang ?? 'zh';
  const note = opts.note ?? '';

  if (opts.localWordbook !== false) {
    const report = await invoke<CollectionPushReport>('add_wordbook_entry', {
      word: opts.word,
      translation: opts.translation,
      fromLang,
      toLang,
      note: note || null,
    });
    return {
      localOk: true,
      report: report ?? { results: [] },
    };
  }

  const report = await invoke<CollectionPushReport>('collection_push', {
    word: opts.word,
    translation: opts.translation,
    note: note || null,
    fromLang,
    toLang,
  });
  return { localOk: true, report };
}

export async function collectionPushOnly(opts: {
  word: string;
  translation: string;
  note?: string;
  fromLang?: string;
  toLang?: string;
}): Promise<CollectionPushReport> {
  return invoke<CollectionPushReport>('collection_push', {
    word: opts.word,
    translation: opts.translation,
    note: opts.note ?? null,
    fromLang: opts.fromLang ?? null,
    toLang: opts.toLang ?? null,
  });
}

export async function collectionTestTarget(target: string): Promise<CollectionPushReport> {
  return invoke<CollectionPushReport>('collection_test_target', { target });
}

export function summarizeReport(report: CollectionPushReport): string {
  if (!report.results.length) {
    return '本地已保存';
  }
  return report.results
    .map((r) => `${r.target}: ${r.ok ? 'OK' : '失败'} — ${r.message}`)
    .join('\n');
}
