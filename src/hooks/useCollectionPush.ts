import { invokeOrThrow } from '../services/invoke';
import type { CollectionPushReport } from '../types';
import { useToastStore } from '../stores/toastStore';

export async function saveAndCollect(opts: {
  word: string;
  translation: string;
  note?: string;
  fromLang?: string;
  toLang?: string;
  /** When true (default), also write local wordbook (auto-push if enabled). */
  localWordbook?: boolean;
  /** When true (default), surface push report via toast store. */
  toast?: boolean;
}): Promise<{ localOk: boolean; report: CollectionPushReport }> {
  const fromLang = opts.fromLang ?? 'en';
  const toLang = opts.toLang ?? 'zh';
  const note = opts.note ?? '';

  let report: CollectionPushReport;

  if (opts.localWordbook !== false) {
    report = (await invokeOrThrow<CollectionPushReport>('add_wordbook_entry', {
      word: opts.word,
      translation: opts.translation,
      fromLang,
      toLang,
      note: note || null,
    })) ?? { results: [] };
  } else {
    report = await invokeOrThrow<CollectionPushReport>('collection_push', {
      word: opts.word,
      translation: opts.translation,
      note: note || null,
      fromLang,
      toLang,
    });
  }

  if (opts.toast !== false) {
    toastCollectionReport(report);
  }

  return { localOk: true, report };
}

export async function collectionPushOnly(opts: {
  word: string;
  translation: string;
  note?: string;
  fromLang?: string;
  toLang?: string;
  toast?: boolean;
}): Promise<CollectionPushReport> {
  const report = await invokeOrThrow<CollectionPushReport>('collection_push', {
    word: opts.word,
    translation: opts.translation,
    note: opts.note ?? null,
    fromLang: opts.fromLang ?? null,
    toLang: opts.toLang ?? null,
  });
  if (opts.toast !== false) {
    toastCollectionReport(report);
  }
  return report;
}

export async function collectionTestTarget(target: string): Promise<CollectionPushReport> {
  return invokeOrThrow<CollectionPushReport>('collection_test_target', { target });
}

export function summarizeReport(report: CollectionPushReport): string {
  if (!report.results.length) {
    return '本地已保存';
  }
  return report.results
    .map((r) => `${r.target}: ${r.ok ? 'OK' : '失败'} — ${r.message}`)
    .join('\n');
}

/** Surface CollectionPushReport via existing toast store (no second toast system). */
export function toastCollectionReport(report: CollectionPushReport): void {
  const addToast = useToastStore.getState().addToast;
  const summary = summarizeReport(report);

  if (!report.results.length) {
    addToast({ type: 'success', message: '已保存到生词本', duration: 2500 });
    return;
  }

  const failed = report.results.filter((r) => !r.ok).length;
  const ok = report.results.length - failed;

  if (failed === 0) {
    addToast({
      type: 'success',
      message: '已保存并外送',
      detail: summary,
      duration: 4000,
    });
  } else if (ok === 0) {
    addToast({
      type: 'warning',
      message: '本地已保存，外送失败',
      detail: summary,
      duration: 5000,
    });
  } else {
    addToast({
      type: 'warning',
      message: '本地已保存，部分外送失败',
      detail: summary,
      duration: 5000,
    });
  }
}
