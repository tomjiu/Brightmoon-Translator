import { useCallback, useState, lazy, Suspense } from 'react';
import { FileUp, Loader2, Book } from 'lucide-react';
import { isTauriRuntime } from '../services/tauriRuntime';
import { setPendingDocPath } from '../services/docHandoff';
import { useI18n } from '../i18n';
import PageHeader from '../components/PageHeader';
// S3-12: viewer subpages lazy-loaded so opening DocumentsViewer doesn't pull
// in PdfViewer/EpubViewer/OfficeViewer/etc. until the user picks a file type.
// Each viewer pulls in heavy deps (pdfium, epub.js, mammoth, xlsx) that are
// wasted payload if the user only ever opens, say, PDFs.
const Glossary = lazy(() => import('./Glossary'));
const PdfViewer = lazy(() => import('./PdfViewer'));
const EpubViewer = lazy(() => import('./EpubViewer'));
const SubtitleViewer = lazy(() => import('./SubtitleViewer'));
// OfficeViewer exports DocxViewer/ExcelViewer/PptxViewer as named exports.
// React.lazy only accepts a default export, so we adapt via .then(). The
// bundler still produces a single chunk for OfficeViewer.
const DocxViewer = lazy(() => import('./OfficeViewer').then((m) => ({ default: m.DocxViewer })));
const ExcelViewer = lazy(() => import('./OfficeViewer').then((m) => ({ default: m.ExcelViewer })));
const PptxViewer = lazy(() => import('./OfficeViewer').then((m) => ({ default: m.PptxViewer })));
const ImageFileTranslate = lazy(() => import('./ImageFileTranslate'));

type DocKind = 'pdf' | 'epub' | 'subtitle' | 'docx' | 'excel' | 'pptx' | 'image';

const EXT_MAP: Record<string, DocKind> = {
  pdf: 'pdf',
  epub: 'epub',
  srt: 'subtitle',
  ass: 'subtitle',
  vtt: 'subtitle',
  docx: 'docx',
  xlsx: 'excel',
  xls: 'excel',
  csv: 'excel',
  pptx: 'pptx',
  png: 'image',
  jpg: 'image',
  jpeg: 'image',
  webp: 'image',
  bmp: 'image',
  gif: 'image',
};

const SUPPORT_HINT = 'PDF · EPUB · Word(.docx) · Excel · PowerPoint · 字幕(.srt/.ass/.vtt) · 图片';

/** Single upload entry — auto-detect format, hand path to existing viewers. */
export default function DocumentsViewer() {
  const isTauri = isTauriRuntime();
  const { t } = useI18n();
  const [mode, setMode] = useState<'home' | DocKind | 'glossary'>('home');
  const [error, setError] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);
  const [fileLabel, setFileLabel] = useState<string | null>(null);

  const openFile = useCallback(async () => {
    if (!isTauri) {
      setError(t('documents.openInDesktop'));
      return;
    }
    setError(null);
    setPicking(true);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: t('documents.translatableDocs'),
            extensions: [
              'pdf',
              'epub',
              'srt',
              'ass',
              'vtt',
              'docx',
              'xlsx',
              'xls',
              'csv',
              'pptx',
              'png',
              'jpg',
              'jpeg',
              'webp',
              'bmp',
              'gif',
            ],
          },
        ],
      });
      if (!selected || typeof selected !== 'string') return;
      const base = selected.split(/[/\\]/).pop() || selected;
      const ext = (base.includes('.') ? base.slice(base.lastIndexOf('.') + 1) : '').toLowerCase();
      const kind = EXT_MAP[ext];
      if (!kind) {
        setError(`暂不支持该格式。支持：${SUPPORT_HINT}`);
        return;
      }
      setPendingDocPath(selected);
      setFileLabel(base);
      setMode(kind);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPicking(false);
    }
  }, [isTauri, t]);

  const backHome = () => {
    setMode('home');
    setFileLabel(null);
    setError(null);
  };

  if (mode === 'glossary') {
    return (
      <div className="h-full flex flex-col">
        <div className="ui-chrome px-4 py-2 border-b border-border flex items-center gap-2 shrink-0">
          <button
            type="button"
            className="text-xs px-2 py-1 rounded border border-border hover:bg-bg-tertiary"
            onClick={backHome}
          >
            返回
          </button>
          <span className="ui-caption">术语表</span>
        </div>
        <div className="flex-1 overflow-hidden">
          <Suspense fallback={<div className="flex items-center justify-center h-full text-text-secondary"><Loader2 className="animate-spin" /></div>}>
            <Glossary />
          </Suspense>
        </div>
      </div>
    );
  }

  if (mode !== 'home') {
    return (
      <div className="h-full flex flex-col">
        <div className="ui-chrome px-4 py-2 border-b border-border flex items-center gap-2 shrink-0">
          <button
            type="button"
            className="text-xs px-2 py-1 rounded border border-border hover:bg-bg-tertiary"
            onClick={backHome}
          >
            返回
          </button>
          {fileLabel && (
            <span className="ui-caption truncate flex-1" title={fileLabel}>
              {fileLabel}
            </span>
          )}
          <button
            type="button"
            className="text-xs px-2 py-1 rounded border border-border hover:bg-bg-tertiary"
            onClick={() => void openFile()}
          >
            换文件
          </button>
        </div>
        <div className="flex-1 overflow-hidden min-h-0">
          <Suspense fallback={<div className="flex items-center justify-center h-full text-text-secondary"><Loader2 className="animate-spin" /></div>}>
            {mode === 'pdf' && <PdfViewer />}
            {mode === 'epub' && <EpubViewer />}
            {mode === 'subtitle' && <SubtitleViewer />}
            {mode === 'docx' && <DocxViewer />}
            {mode === 'excel' && <ExcelViewer />}
            {mode === 'pptx' && <PptxViewer />}
            {mode === 'image' && <ImageFileTranslate />}
          </Suspense>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto p-4 md:p-5 lg:p-6 flex flex-col">
      <div className="max-w-3xl mx-auto space-y-6 w-full">
        <PageHeader
          title="文档翻译"
          description="选择文件后自动识别类型并处理，无需切换多个入口。"
          icon={FileUp}
        />

        <section className="ui-card ui-card-hover p-5 md:p-6 space-y-4">
          <div className="space-y-1">
            <p className="ui-section-title">支持格式</p>
            <p className="ui-caption leading-relaxed">{SUPPORT_HINT}</p>
          </div>
          <button
            type="button"
            disabled={picking || !isTauri}
            onClick={() => void openFile()}
            className="w-full flex flex-col items-center justify-center gap-2 py-7 rounded-xl border border-dashed border-border-strong hover:bg-bg-tertiary transition-colors disabled:opacity-50"
          >
            {picking ? (
              <Loader2 className="w-8 h-8 animate-spin text-primary" />
            ) : (
              <FileUp className="w-8 h-8 text-text-secondary" />
            )}
            <span className="ui-section-title">{picking ? '正在打开…' : '选择文件'}</span>
            <span className="ui-caption leading-relaxed">上传后自动进入对应处理流程</span>
          </button>
          {error && <p className="ui-caption text-red-400">{error}</p>}
          {!isTauri && <p className="ui-caption leading-relaxed">请在桌面客户端中使用文件翻译</p>}
        </section>

        <button
          type="button"
          onClick={() => setMode('glossary')}
          className="flex items-center gap-2 ui-caption hover:text-text-primary"
        >
          <Book size={14} />
          管理术语表
        </button>
      </div>
    </div>
  );
}
