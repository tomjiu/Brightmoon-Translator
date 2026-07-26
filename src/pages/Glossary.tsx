import { useState, useEffect, useCallback } from 'react';
import { invokeOrThrow } from '../services/invoke';
import { Plus, Trash2, Book, Download, Upload, FileText, FileSpreadsheet } from 'lucide-react';
import { useToastStore } from '../stores/toastStore';
import { useI18n } from '../i18n';

interface GlossaryEntry {
  source: string;
  target: string;
  context?: string;
}

const LANG_PAIRS = [
  { value: 'en-zh', labelKey: 'glossary.enToZh', fallback: '英 → 中' },
  { value: 'zh-en', labelKey: 'glossary.zhToEn', fallback: '中 → 英' },
  { value: 'ja-zh', labelKey: 'glossary.jaToZh', fallback: '日 → 中' },
  { value: 'zh-ja', labelKey: 'glossary.zhToJa', fallback: '中 → 日' },
  { value: 'ko-zh', labelKey: 'glossary.koToZh', fallback: '韩 → 中' },
  { value: 'zh-ko', labelKey: 'glossary.zhToKo', fallback: '中 → 韩' },
];

function Glossary() {
  const [entries, setEntries] = useState<Record<string, GlossaryEntry[]>>({});
  const [langPair, setLangPair] = useState('en-zh');
  const [newSource, setNewSource] = useState('');
  const [newTarget, setNewTarget] = useState('');
  const [newContext, setNewContext] = useState('');
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exporting, setExporting] = useState(false);

  const addToast = useToastStore((state) => state.addToast);
  const { t } = useI18n();

  useEffect(() => {
    loadGlossary();
  }, []);

  const loadGlossary = async () => {
    try {
      const allEntries = await invokeOrThrow<Record<string, GlossaryEntry[]>>('get_all_glossary');
      setEntries(allEntries);
    } catch (err) {
      console.error('Failed to load glossary:', err);
    }
  };

  const addEntry = async () => {
    if (!newSource.trim() || !newTarget.trim()) return;

    setLoading(true);
    try {
      await invokeOrThrow('add_glossary_entry', {
        langPair,
        source: newSource.trim(),
        target: newTarget.trim(),
        context: newContext.trim() || null,
      });
      setNewSource('');
      setNewTarget('');
      setNewContext('');
      await loadGlossary();
    } catch (err) {
      console.error('Failed to add entry:', err);
    } finally {
      setLoading(false);
    }
  };

  const removeEntry = async (langPair: string, source: string) => {
    try {
      await invokeOrThrow('remove_glossary_entry', { langPair, source });
      await loadGlossary();
    } catch (err) {
      console.error('Failed to remove entry:', err);
    }
  };

  const handleImportTmx = useCallback(async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.tmx,.xml';

    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setImporting(true);
      try {
        const text = await file.text();
        const result = await invokeOrThrow<[number, number]>('import_glossary_tmx', {
          xml: text,
        });
        await loadGlossary();
        addToast({
          type: 'success',
          message:
            t('glossary.tmxImportSuccess', { count: result[0] }) ||
            `TMX 导入成功: ${result[0]} 条术语`,
          duration: 3000,
        });
      } catch (err) {
        console.error('Failed to import TMX:', err);
        addToast({
          type: 'error',
          message: t('glossary.tmxImportFailed', { error: String(err) }) || `TMX 导入失败: ${err}`,
          duration: 4000,
        });
      } finally {
        setImporting(false);
      }
    };

    input.click();
  }, [loadGlossary, addToast]);

  const handleImportTbx = useCallback(async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.tbx,.xml';

    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setImporting(true);
      try {
        const text = await file.text();
        const result = await invokeOrThrow<[number, number]>('import_glossary_tbx', {
          xml: text,
        });
        await loadGlossary();
        addToast({
          type: 'success',
          message:
            t('glossary.tbxImportSuccess', { count: result[0] }) ||
            `TBX 导入成功: ${result[0]} 条术语`,
          duration: 3000,
        });
      } catch (err) {
        console.error('Failed to import TBX:', err);
        addToast({
          type: 'error',
          message: t('glossary.tbxImportFailed', { error: String(err) }) || `TBX 导入失败: ${err}`,
          duration: 4000,
        });
      } finally {
        setImporting(false);
      }
    };

    input.click();
  }, [loadGlossary, addToast]);

  const handleExportTmx = useCallback(async () => {
    setExporting(true);
    try {
      const xml = await invokeOrThrow<string>('export_glossary_tmx', {
        langPair: null,
      });
      const blob = new Blob([xml], { type: 'application/xml' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `glossary-${new Date().toISOString().slice(0, 10)}.tmx`;
      a.click();
      URL.revokeObjectURL(url);
      addToast({
        type: 'success',
        message: t('glossary.tmxExportSuccess') || 'TMX 导出成功',
        duration: 3000,
      });
    } catch (err) {
      console.error('Failed to export TMX:', err);
      addToast({
        type: 'error',
        message: t('glossary.tmxExportFailed', { error: String(err) }) || `TMX 导出失败: ${err}`,
        duration: 4000,
      });
    } finally {
      setExporting(false);
    }
  }, [addToast]);

  const handleExportTbx = useCallback(async () => {
    setExporting(true);
    try {
      const xml = await invokeOrThrow<string>('export_glossary_tbx', {
        langPair: null,
      });
      const blob = new Blob([xml], { type: 'application/xml' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `glossary-${new Date().toISOString().slice(0, 10)}.tbx`;
      a.click();
      URL.revokeObjectURL(url);
      addToast({
        type: 'success',
        message: t('glossary.tbxExportSuccess') || 'TBX 导出成功',
        duration: 3000,
      });
    } catch (err) {
      console.error('Failed to export TBX:', err);
      addToast({
        type: 'error',
        message: t('glossary.tbxExportFailed', { error: String(err) }) || `TBX 导出失败: ${err}`,
        duration: 4000,
      });
    } finally {
      setExporting(false);
    }
  }, [addToast]);

  return (
    <div className="flex flex-col h-full p-6">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <Book size={24} className="text-primary" />
          <h1 className="text-xl font-bold text-text-primary">
            {t('glossary.title') || '术语表管理'}
          </h1>
        </div>

        {/* Import/Export Buttons */}
        <div className="flex gap-2">
          <button
            onClick={handleImportTmx}
            disabled={importing}
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            <Upload size={14} />
            <FileText size={14} />
            {importing
              ? t('glossary.importing') || '导入中...'
              : t('glossary.importTmx') || '导入 TMX'}
          </button>
          <button
            onClick={handleImportTbx}
            disabled={importing}
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            <Upload size={14} />
            <FileSpreadsheet size={14} />
            {importing
              ? t('glossary.importing') || '导入中...'
              : t('glossary.importTbx') || '导入 TBX'}
          </button>
          <button
            onClick={handleExportTmx}
            disabled={exporting || Object.keys(entries).length === 0}
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-primary hover:text-primary-fg hover:border-primary transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            <Download size={14} />
            <FileText size={14} />
            {exporting
              ? t('glossary.exporting') || '导出中...'
              : t('glossary.exportTmx') || '导出 TMX'}
          </button>
          <button
            onClick={handleExportTbx}
            disabled={exporting || Object.keys(entries).length === 0}
            className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-primary hover:text-primary-fg hover:border-primary transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            <Download size={14} />
            <FileSpreadsheet size={14} />
            {exporting
              ? t('glossary.exporting') || '导出中...'
              : t('glossary.exportTbx') || '导出 TBX'}
          </button>
        </div>
      </div>

      {/* Add Entry Form */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4 mb-6">
        <h2 className="text-sm font-semibold text-text-secondary mb-3">
          {t('glossary.addTerm') || '添加术语'}
        </h2>
        <div className="flex gap-3">
          <select
            value={langPair}
            onChange={(e) => setLangPair(e.target.value)}
            className="bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          >
            {LANG_PAIRS.map((lp) => (
              <option key={lp.value} value={lp.value}>
                {t(lp.labelKey) || lp.fallback}
              </option>
            ))}
          </select>
          <input
            type="text"
            value={newSource}
            onChange={(e) => setNewSource(e.target.value)}
            placeholder={t('common.sourceText') || '原文'}
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <input
            type="text"
            value={newTarget}
            onChange={(e) => setNewTarget(e.target.value)}
            placeholder={t('common.targetText') || '译文'}
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <input
            type="text"
            value={newContext}
            onChange={(e) => setNewContext(e.target.value)}
            placeholder={t('glossary.contextOptional') || '上下文(可选)'}
            className="flex-1 bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
          />
          <button
            onClick={addEntry}
            disabled={loading || !newSource.trim() || !newTarget.trim()}
            className="bg-primary text-bg-primary rounded-lg px-4 py-2 text-sm font-semibold hover:bg-primary-hover transition-colors disabled:opacity-50 flex items-center gap-2"
          >
            <Plus size={16} />
            {t('glossary.add') || '添加'}
          </button>
        </div>
      </div>

      {/* Glossary Entries */}
      <div className="flex-1 overflow-y-auto">
        {Object.keys(entries).length === 0 ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            {t('glossary.noEntries') || '暂无术语条目'}
          </div>
        ) : (
          Object.entries(entries).map(([pair, pairEntries]) => (
            <div
              key={pair}
              className="bg-bg-secondary border border-border rounded-xl mb-4 overflow-hidden"
            >
              <div className="bg-bg-tertiary px-4 py-2 border-b border-border">
                <span className="text-sm font-semibold text-primary">
                  {(() => {
                    const lp = LANG_PAIRS.find((lp) => lp.value === pair);
                    return lp ? t(lp.labelKey) || lp.fallback : pair;
                  })()}
                </span>
                <span className="text-xs text-text-secondary ml-2">
                  ({pairEntries.length} {t('glossary.entriesCount') || '条'})
                </span>
              </div>
              <div className="divide-y divide-border">
                {pairEntries.map((entry, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between px-4 py-3 hover:bg-bg-tertiary/50"
                  >
                    <div className="flex-1">
                      <span className="text-sm text-text-primary font-medium">{entry.source}</span>
                      <span className="text-text-secondary mx-2">→</span>
                      <span className="text-sm text-primary">{entry.target}</span>
                      {entry.context && (
                        <span className="text-xs text-text-secondary ml-2">({entry.context})</span>
                      )}
                    </div>
                    <button
                      onClick={() => removeEntry(pair, entry.source)}
                      className="text-text-secondary hover:text-error transition-colors p-1"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default Glossary;
