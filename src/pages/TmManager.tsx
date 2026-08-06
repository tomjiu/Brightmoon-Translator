import { useState, useEffect } from 'react';
import { useI18n } from '../i18n';
import { useToastStore } from '../stores/toastStore';
import { invokeOrThrow, safeInvoke } from '../services/invoke';
import { isTauriRuntime } from '../services/tauriRuntime';
import {
  Search,
  Trash2,
  Upload,
  Database,
  ChevronLeft,
  ChevronRight,
  X,
  FileJson,
  FileText,
  Loader2,
  Check,
} from 'lucide-react';
import PageHeader from '../components/PageHeader';
import Icon from '../components/Icon';

// ── Types ───────────────────────────────────────────────────────────────────────

interface TmExportEntry {
  source: string;
  target: string;
  fromLang: string;
  toLang: string;
  engine: string;
  timestamp: number;
}

interface TmStats {
  total: number;
  langPairs: Array<[string, string, number]>;
}

// ── Component ───────────────────────────────────────────────────────────────────

function TmManager() {
  const { t } = useI18n();
  const addToast = useToastStore((s) => s.addToast);
  const isTauri = isTauriRuntime();

  // State
  const [stats, setStats] = useState<TmStats | null>(null);
  const [entries, setEntries] = useState<TmExportEntry[]>([]);
  const [totalResults, setTotalResults] = useState(0);
  const [searchQuery, setSearchQuery] = useState('');
  const [fromLang, setFromLang] = useState('');
  const [toLang, setToLang] = useState('');
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [selected, setSelected] = useState<Set<number>>(new Set());

  const pageSize = 50;

  // Load stats on mount
  useEffect(() => {
    if (!isTauri) return;

    loadStats();
  }, [isTauri]);

  // Search when filters change
  useEffect(() => {
    if (!isTauri) return;

    searchEntries();
  }, [searchQuery, fromLang, toLang, page, isTauri]);

  const loadStats = async () => {
    if (!isTauri) return;

    try {
      const data = await invokeOrThrow<TmStats>('tm_get_stats');
      setStats(data);
    } catch {
      // Error already shown by invokeOrThrow
    }
  };

  const searchEntries = async () => {
    if (!isTauri) return;

    setLoading(true);
    try {
      const [data, err] = await safeInvoke<[TmExportEntry[], number]>('tm_search', {
        query: searchQuery,
        fromLang: fromLang || undefined,
        toLang: toLang || undefined,
        limit: pageSize,
        offset: page * pageSize,
      });
      if (!err && data) {
        setEntries(data[0]);
        setTotalResults(data[1]);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async (entry: TmExportEntry) => {
    // eslint-disable-next-line no-alert
    if (!confirm(t('tm.deleteConfirm'))) return;
    try {
      await invokeOrThrow<number>('tm_delete', {
        source: entry.source,
        target: entry.target,
        fromLang: entry.fromLang,
        toLang: entry.toLang,
      });
      setSelected(new Set());
      await searchEntries();
      await loadStats();
    } catch {
      // Error shown by invokeOrThrow
    }
  };

  const handleBatchDelete = async () => {
    if (selected.size === 0) return;
    // eslint-disable-next-line no-alert
    if (!confirm(t('tm.batchDeleteConfirm', { count: selected.size }))) return;
    try {
      const entriesToDelete = Array.from(selected).map((i) => [
        entries[i].source,
        entries[i].target,
        entries[i].fromLang,
        entries[i].toLang,
      ]);
      await invokeOrThrow<number>('tm_batch_delete', { entries: entriesToDelete });
      setSelected(new Set());
      await searchEntries();
      await loadStats();
    } catch {
      // Error shown by invokeOrThrow
    }
  };

  const handleExportJson = async () => {
    setExporting(true);
    try {
      const json = await invokeOrThrow<string>('tm_export', {
        fromLang: fromLang || undefined,
        toLang: toLang || undefined,
      });
      downloadFile(json, 'tm-export.json', 'application/json');
    } finally {
      setExporting(false);
    }
  };

  const handleExportTmx = async () => {
    setExporting(true);
    try {
      const xml = await invokeOrThrow<string>('tm_export_tmx', {
        fromLang: fromLang || undefined,
        toLang: toLang || undefined,
      });
      downloadFile(xml, 'tm-export.tmx', 'application/xml');
    } finally {
      setExporting(false);
    }
  };

  const handleImportJson = async () => {
    const content = await selectFile('.json');
    if (!content) return;
    setImporting(true);
    try {
      const [imported, duplicates] = await invokeOrThrow<[number, number]>('tm_import', {
        json: content,
        deduplicate: true,
      });
      addToast({
        type: 'success',
        message: `${t('tm.imported') || '导入'}: ${imported}, ${t('tm.duplicatesSkipped') || '跳过重复'}: ${duplicates}`,
        duration: 3000,
      });
      await searchEntries();
      await loadStats();
    } finally {
      setImporting(false);
    }
  };

  const handleImportTmx = async () => {
    const content = await selectFile('.tmx,.xml');
    if (!content) return;
    setImporting(true);
    try {
      const [imported, duplicates] = await invokeOrThrow<[number, number]>('tm_import_tmx', {
        xml: content,
        deduplicate: true,
      });
      addToast({
        type: 'success',
        message: `${t('tm.imported') || '导入'}: ${imported}, ${t('tm.duplicatesSkipped') || '跳过重复'}: ${duplicates}`,
        duration: 3000,
      });
      await searchEntries();
      await loadStats();
    } finally {
      setImporting(false);
    }
  };

  const toggleSelect = (index: number) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (selected.size === entries.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(entries.map((_, i) => i)));
    }
  };

  const totalPages = Math.ceil(totalResults / pageSize);
  const uniqueLangs = [...new Set(stats?.langPairs.flatMap(([a, b]) => [a, b]) ?? [])].sort();

  return (
    <div className="h-full overflow-y-auto p-6">
      <div className="max-w-5xl mx-auto">
        {/* Header */}
        <PageHeader
          title={t('tm.title')}
          icon={Database}
          actions={
            <div className="flex gap-2 flex-wrap">
              <button
                onClick={handleExportJson}
                disabled={exporting}
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                {exporting ? (
                  <Icon icon={Loader2} size="sm" className="animate-spin" />
                ) : (
                  <Icon icon={FileJson} size="sm" />
                )}
                {t('tm.exportJson')}
              </button>
              <button
                onClick={handleExportTmx}
                disabled={exporting}
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                {exporting ? (
                  <Icon icon={Loader2} size="sm" className="animate-spin" />
                ) : (
                  <Icon icon={FileText} size="sm" />
                )}
                {t('tm.exportTmx')}
              </button>
              <button
                onClick={handleImportJson}
                disabled={importing}
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                {importing ? (
                  <Icon icon={Loader2} size="sm" className="animate-spin" />
                ) : (
                  <Icon icon={Upload} size="sm" />
                )}
                {t('tm.importJson')}
              </button>
              <button
                onClick={handleImportTmx}
                disabled={importing}
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                {importing ? (
                  <Icon icon={Loader2} size="sm" className="animate-spin" />
                ) : (
                  <Icon icon={Upload} size="sm" />
                )}
                {t('tm.importTmx')}
              </button>
            </div>
          }
        />

        {/* Statistics */}
        {stats && (
          <div className="ui-card p-4 mb-6">
            <h2 className="text-sm font-semibold text-text-primary mb-3">{t('tm.statistics')}</h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div>
                <p className="text-xs text-text-secondary">{t('tm.totalEntries')}</p>
                <p className="text-lg font-bold text-primary">{stats.total.toLocaleString()}</p>
              </div>
              {stats.langPairs.slice(0, 3).map(([from, to, count]) => (
                <div key={`${from}-${to}`}>
                  <p className="text-xs text-text-secondary">{`${from} → ${to}`}</p>
                  <p className="text-lg font-bold text-text-primary">{count.toLocaleString()}</p>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Search & Filters */}
        <div className="ui-card p-4 mb-6">
          <div className="flex flex-col md:flex-row gap-3">
            <div className="relative flex-1">
              <Search
                size={16}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-text-secondary"
              />
              <input
                type="text"
                value={searchQuery}
                onChange={(e) => {
                  setSearchQuery(e.target.value);
                  setPage(0);
                }}
                placeholder={t('tm.searchPlaceholder')}
                className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg pl-10 pr-4 py-2 text-sm focus:outline-none focus:border-primary"
              />
              {searchQuery && (
                <button
                  onClick={() => {
                    setSearchQuery('');
                    setPage(0);
                  }}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
                >
                  <X size={14} />
                </button>
              )}
            </div>
            <select
              value={fromLang}
              onChange={(e) => {
                setFromLang(e.target.value);
                setPage(0);
              }}
              className="bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary"
            >
              <option value="">{t('tm.anySourceLang')}</option>
              {uniqueLangs.map((lang) => (
                <option key={lang} value={lang}>
                  {lang}
                </option>
              ))}
            </select>
            <select
              value={toLang}
              onChange={(e) => {
                setToLang(e.target.value);
                setPage(0);
              }}
              className="bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:outline-none focus:border-primary"
            >
              <option value="">{t('tm.anyTargetLang')}</option>
              {uniqueLangs.map((lang) => (
                <option key={lang} value={lang}>
                  {lang}
                </option>
              ))}
            </select>
          </div>
        </div>

        {/* Results Info & Batch Actions */}
        <div className="flex items-center justify-between mb-3">
          <p className="text-sm text-text-secondary">
            {t('tm.resultsFound', { count: totalResults })}
            {totalPages > 1 && ` · ${t('tm.pageInfo', { current: page + 1, total: totalPages })}`}
          </p>
          {selected.size > 0 && (
            <div className="flex items-center gap-3">
              <span className="text-sm text-primary">
                {t('tm.selected', { count: selected.size })}
              </span>
              <button
                onClick={handleBatchDelete}
                className="bg-error/10 text-error text-xs px-3 py-1.5 rounded-lg hover:bg-error/20 transition-colors flex items-center gap-1"
              >
                <Trash2 size={12} />
                {t('tm.batchDelete')}
              </button>
            </div>
          )}
        </div>

        {/* Table */}
        <div className="ui-card overflow-hidden mb-6">
          {loading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 size={24} className="animate-spin text-primary" />
            </div>
          ) : entries.length === 0 ? (
            <div className="text-center py-12 text-text-secondary">
              <Database size={48} className="mx-auto mb-3 opacity-30" />
              <p>{t('tm.noResults')}</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border bg-bg-tertiary/50">
                    <th className="px-4 py-3 text-left">
                      <button
                        onClick={toggleSelectAll}
                        className="w-4 h-4 rounded border border-border accent-primary"
                      >
                        {selected.size === entries.length && entries.length > 0 && (
                          <Check size={12} className="text-primary" />
                        )}
                      </button>
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-text-secondary">
                      {t('tm.source')}
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-text-secondary">
                      {t('tm.target')}
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-text-secondary">
                      {t('tm.engine')}
                    </th>
                    <th className="px-4 py-3 text-left text-xs font-medium text-text-secondary">
                      {t('tm.time')}
                    </th>
                    <th className="px-4 py-3 text-right text-xs font-medium text-text-secondary">
                      {t('tm.actions')}
                    </th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {entries.map((entry, i) => (
                    <tr key={i} className="hover:bg-bg-tertiary/30 transition-colors">
                      <td className="px-4 py-3">
                        <button
                          onClick={() => toggleSelect(i)}
                          className={`w-4 h-4 rounded border transition-colors ${
                            selected.has(i) ? 'bg-primary border-primary' : 'border-border'
                          }`}
                        >
                          {selected.has(i) && <Check size={12} className="text-white" />}
                        </button>
                      </td>
                      <td
                        className="px-4 py-3 max-w-[200px] truncate text-text-primary"
                        title={entry.source}
                      >
                        {entry.source}
                      </td>
                      <td
                        className="px-4 py-3 max-w-[200px] truncate text-text-primary"
                        title={entry.target}
                      >
                        {entry.target}
                      </td>
                      <td className="px-4 py-3 text-text-secondary">
                        <span className="text-xs bg-bg-tertiary px-2 py-0.5 rounded">
                          {entry.engine}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-text-secondary text-xs">
                        {new Date(entry.timestamp).toLocaleString()}
                      </td>
                      <td className="px-4 py-3 text-right">
                        <button
                          onClick={() => handleDelete(entry)}
                          className="text-text-secondary hover:text-error transition-colors p-1"
                          title={t('tm.delete')}
                        >
                          <Trash2 size={14} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Pagination */}
        {totalPages > 1 && (
          <div className="flex items-center justify-center gap-2">
            <button
              onClick={() => setPage((p) => Math.max(0, p - 1))}
              disabled={page === 0}
              className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            >
              <ChevronLeft size={16} />
            </button>
            <span className="text-sm text-text-secondary px-3">
              {page + 1} / {totalPages}
            </span>
            <button
              onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
              disabled={page >= totalPages - 1}
              className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            >
              <ChevronRight size={16} />
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Helpers ─────────────────────────────────────────────────────────────────────

function downloadFile(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function selectFile(accept: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return resolve(null);
      const text = await file.text();
      resolve(text);
    };
    input.click();
  });
}

export default TmManager;
