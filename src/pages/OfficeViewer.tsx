import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invokeOrThrow } from '../services/invoke';
import { useI18n } from '../i18n';
import { isTauriRuntime } from '../services/tauriRuntime';
import { FileText, Languages, Download, Loader2 } from 'lucide-react';
import PageLayout from '../components/PageLayout';
import { takePendingDocPath } from '../services/docHandoff';

type OfficeKind = 'docx' | 'excel' | 'pptx';

interface DocxParagraph {
  index: number;
  text: string;
  style: string;
  isHeading: boolean;
  headingLevel: number;
}

interface DocxDocument {
  title: string;
  paragraphs: DocxParagraph[];
  totalParagraphs: number;
  totalWords: number;
}

interface TranslatedParagraph {
  index: number;
  originalText: string;
  translatedText: string;
  style: string;
  isHeading: boolean;
  headingLevel: number;
}

interface TranslatedDocx {
  title: string;
  paragraphs: TranslatedParagraph[];
  totalParagraphs: number;
  totalWords: number;
}

interface ExcelCell {
  row: number;
  col: number;
  text: string;
  isFormula: boolean;
}

interface ExcelSheet {
  name: string;
  cells: ExcelCell[];
  totalCells: number;
  totalWords: number;
}

interface ExcelDocument {
  title: string;
  sheets: ExcelSheet[];
  totalSheets: number;
  totalCells: number;
  totalWords: number;
}

interface TranslatedCell {
  row: number;
  col: number;
  originalText: string;
  translatedText: string;
  isFormula: boolean;
}

interface TranslatedSheet {
  name: string;
  cells: TranslatedCell[];
}

interface TranslatedExcel {
  title: string;
  sheets: TranslatedSheet[];
}

interface PptxTextBlock {
  id: string;
  text: string;
  slideIndex: number;
}

interface PptxSlide {
  index: number;
  name: string;
  textBlocks: PptxTextBlock[];
}

interface PptxDocument {
  title: string;
  slides: PptxSlide[];
  totalSlides: number;
  totalTextBlocks: number;
  totalWords: number;
}

interface TranslatedTextBlock {
  id: string;
  originalText: string;
  translatedText: string;
  slideIndex: number;
}

interface TranslatedSlide {
  index: number;
  name: string;
  textBlocks: TranslatedTextBlock[];
}

interface TranslatedPptx {
  title: string;
  slides: TranslatedSlide[];
}

interface ProgressInfo {
  stage?: string;
  [key: string]: unknown;
}

const KIND_META: Record<
  OfficeKind,
  {
    ext: string[];
    open: string;
    preview: string;
    translate: string;
    progress: string;
    label: string;
  }
> = {
  docx: {
    ext: ['docx'],
    open: 'open_docx',
    preview: 'translate_docx_preview',
    translate: 'translate_docx',
    progress: 'docx-progress',
    label: 'Word',
  },
  excel: {
    ext: ['xlsx', 'xls'],
    open: 'open_excel',
    preview: 'translate_excel_preview',
    translate: 'translate_excel',
    progress: 'excel-progress',
    label: 'Excel',
  },
  pptx: {
    ext: ['pptx'],
    open: 'open_pptx',
    preview: 'translate_pptx_preview',
    translate: 'translate_pptx',
    progress: 'pptx-progress',
    label: 'PowerPoint',
  },
};

function OfficeViewer({ kind }: { kind: OfficeKind }) {
  const meta = KIND_META[kind];
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState('');
  const [doc, setDoc] = useState<DocxDocument | ExcelDocument | PptxDocument | null>(null);
  const [preview, setPreview] = useState<TranslatedDocx | TranslatedExcel | TranslatedPptx | null>(
    null,
  );
  const [loading, setLoading] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [progress, setProgress] = useState<ProgressInfo | null>(null);
  const [fromLang, setFromLang] = useState('auto');
  const [toLang, setToLang] = useState('zh');
  const { t } = useI18n();
  const isTauri = isTauriRuntime();

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    listen<ProgressInfo>(meta.progress, (e) => setProgress(e.payload)).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [isTauri, meta.progress]);

  useEffect(() => {
    if (!isTauri) return;
    const path = takePendingDocPath();
    if (!path) return;
    setFilePath(path);
    setFileName(path.split(/[/\\]/).pop() || 'file');
    setPreview(null);
    setProgress(null);
    setLoading(true);
    void invokeOrThrow<DocxDocument | ExcelDocument | PptxDocument>(meta.open, { filePath: path })
      .then((opened) => setDoc(opened))
      .catch((err: unknown) => console.error('open office file failed:', err))
      .finally(() => setLoading(false));
  }, [isTauri, meta.open]);

  const openFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{ name: meta.label, extensions: meta.ext }],
      });
      if (!selected) return;
      const path = selected;
      setFilePath(path);
      setFileName(path.split(/[/\\]/).pop() || 'file');
      setPreview(null);
      setProgress(null);
      setLoading(true);
      try {
        const opened = await invokeOrThrow<DocxDocument | ExcelDocument | PptxDocument>(meta.open, {
          filePath: path,
        });
        setDoc(opened);
      } finally {
        setLoading(false);
      }
    } catch (err) {
      console.error('open office file failed:', err);
    }
  };

  const runPreview = async () => {
    if (!filePath) return;
    setTranslating(true);
    setProgress(null);
    try {
      const result = await invokeOrThrow(meta.preview, {
        inputPath: filePath,
        fromLang,
        toLang,
      });
      setPreview(result as TranslatedDocx | TranslatedExcel | TranslatedPptx);
    } catch (err) {
      console.error('preview translate failed:', err);
    } finally {
      setTranslating(false);
    }
  };

  const exportTranslated = async () => {
    if (!filePath) return;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const ext = meta.ext[0];
      const defaultName = fileName.replace(/\.[^.]+$/, `_translated.${ext}`);
      const outputPath = await save({
        defaultPath: defaultName,
        filters: [{ name: meta.label, extensions: [ext] }],
      });
      if (!outputPath) return;
      setExporting(true);
      setProgress(null);
      await invokeOrThrow(meta.translate, {
        inputPath: filePath,
        outputPath,
        fromLang,
        toLang,
      });
    } catch (err) {
      console.error('export office translate failed:', err);
    } finally {
      setExporting(false);
    }
  };

  const lines = renderLines(kind, doc, preview);

  return (
    <PageLayout
      title={meta.label}
      icon={FileText}
      scrollable={false}
      contentClassName="p-6 flex flex-col"
      actions={
          <div className="flex items-center gap-2 flex-wrap">
            <select
              value={fromLang}
              onChange={(e) => setFromLang(e.target.value)}
              className="bg-bg-secondary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
            >
              <option value="auto">Auto</option>
              <option value="en">English</option>
              <option value="zh">中文</option>
              <option value="ja">日本語</option>
              <option value="ko">한국어</option>
            </select>
            <span className="text-text-secondary">→</span>
            <select
              value={toLang}
              onChange={(e) => setToLang(e.target.value)}
              className="bg-bg-secondary text-text-primary border border-border rounded-lg px-3 py-2 text-sm"
            >
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="ja">日本語</option>
              <option value="ko">한국어</option>
            </select>
            <button
              type="button"
              onClick={openFile}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-bg-secondary border border-border text-sm hover:bg-bg-tertiary"
            >
              <FileText size={16} />
              {t('common.open') || '打开'}
            </button>
            <button
              type="button"
              onClick={runPreview}
              disabled={!filePath || translating}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-primary text-primary-fg text-sm disabled:opacity-50"
            >
              {translating ? (
                <Loader2 size={16} className="animate-spin" />
              ) : (
                <Languages size={16} />
              )}
              {t('common.translate') || '预览翻译'}
            </button>
            <button
              type="button"
              onClick={exportTranslated}
              disabled={!filePath || exporting}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-bg-secondary border border-border text-sm hover:bg-bg-tertiary disabled:opacity-50"
            >
              {exporting ? <Loader2 size={16} className="animate-spin" /> : <Download size={16} />}
              {t('common.export') || '导出译文'}
            </button>
          </div>
        }
      >

      {fileName && (
        <p className="text-sm text-text-secondary mb-2 truncate" title={filePath || undefined}>
          {fileName}
        </p>
      )}
      {progress?.stage && (
        <p className="text-xs text-text-tertiary mb-2">
          {String(progress.stage)}
          {progress.paragraphsTranslated != null
            ? ` · ${String(progress.paragraphsTranslated)}`
            : ''}
        </p>
      )}

      <div className="flex-1 overflow-auto min-h-0 rounded-lg border border-border bg-bg-secondary p-4 space-y-3">
        {loading && (
          <div className="flex items-center gap-2 text-text-secondary">
            <Loader2 className="animate-spin" size={18} />
            Loading…
          </div>
        )}
        {!loading && !doc && (
          <p className="text-text-secondary text-sm">打开 {meta.label} 文件以提取文本并翻译。</p>
        )}
        {lines.map((line, i) => (
          <div key={i} className="border-b border-border/50 pb-2 last:border-0">
            {line.meta && <div className="text-xs text-text-tertiary mb-0.5">{line.meta}</div>}
            <div className="text-sm text-text-primary whitespace-pre-wrap">{line.original}</div>
            {line.translated ? (
              <div className="text-sm text-primary mt-1 whitespace-pre-wrap">{line.translated}</div>
            ) : null}
          </div>
        ))}
      </div>
    </PageLayout>
  );
}

function renderLines(
  kind: OfficeKind,
  doc: DocxDocument | ExcelDocument | PptxDocument | null,
  preview: TranslatedDocx | TranslatedExcel | TranslatedPptx | null,
): Array<{ meta?: string; original: string; translated?: string }> {
  if (!doc) return [];
  if (kind === 'docx') {
    const d = doc as DocxDocument;
    const p = preview as TranslatedDocx | null;
    if (p?.paragraphs.length) {
      return p.paragraphs
        .filter((x) => x.originalText.trim())
        .map((x) => ({
          meta: x.isHeading ? `H${x.headingLevel}` : undefined,
          original: x.originalText,
          translated: x.translatedText || undefined,
        }));
    }
    return d.paragraphs
      .filter((x) => x.text.trim())
      .map((x) => ({
        meta: x.isHeading ? `H${x.headingLevel}` : undefined,
        original: x.text,
      }));
  }
  if (kind === 'excel') {
    const d = doc as ExcelDocument;
    const p = preview as TranslatedExcel | null;
    if (p?.sheets.length) {
      return p.sheets.flatMap((sheet) =>
        sheet.cells
          .filter((c) => c.originalText.trim())
          .map((c) => ({
            meta: `${sheet.name} R${c.row}C${c.col}`,
            original: c.originalText,
            translated: c.translatedText || undefined,
          })),
      );
    }
    return d.sheets.flatMap((sheet) =>
      sheet.cells
        .filter((c) => c.text.trim())
        .map((c) => ({
          meta: `${sheet.name} R${c.row}C${c.col}`,
          original: c.text,
        })),
    );
  }
  const d = doc as PptxDocument;
  const p = preview as TranslatedPptx | null;
  if (p?.slides.length) {
    return p.slides.flatMap((slide) =>
      slide.textBlocks
        .filter((b) => b.originalText.trim())
        .map((b) => ({
          meta: `Slide ${slide.index + 1}`,
          original: b.originalText,
          translated: b.translatedText || undefined,
        })),
    );
  }
  return d.slides.flatMap((slide) =>
    slide.textBlocks
      .filter((b) => b.text.trim())
      .map((b) => ({
        meta: `Slide ${slide.index + 1}`,
        original: b.text,
      })),
  );
}

export function DocxViewer() {
  return <OfficeViewer kind="docx" />;
}

export function ExcelViewer() {
  return <OfficeViewer kind="excel" />;
}

export function PptxViewer() {
  return <OfficeViewer kind="pptx" />;
}

export default OfficeViewer;
