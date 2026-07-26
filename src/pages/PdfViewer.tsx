import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invokeOrThrow } from '../services/invoke';
import { useI18n } from '../i18n';
import { isTauriRuntime } from '../services/tauriRuntime';
import {
  FileText,
  Languages,
  Download,
  ChevronLeft,
  ChevronRight,
  ScanLine,
  Loader2,
} from 'lucide-react';

interface PdfPage {
  pageNumber: number;
  text: string;
}

interface TranslatedPage {
  pageNumber: number;
  originalText: string;
  translatedText: string;
}

interface PdfDocument {
  pages: PdfPage[];
  totalPages: number;
  isScanned: boolean;
}

interface TranslatedPdf {
  pages: TranslatedPage[];
  totalPages: number;
  isScanned: boolean;
}

interface ScannedPdfOcrResult {
  pages: PdfPage[];
  totalPages: number;
  processedPages: number;
}

interface OcrProgress {
  current: number;
  total: number;
  done?: boolean;
}

function PdfViewer() {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string>('');
  const [pdfDoc, setPdfDoc] = useState<PdfDocument | null>(null);
  const [translatedPdf, setTranslatedPdf] = useState<TranslatedPdf | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [showBilingual, setShowBilingual] = useState(true);
  const [fromLang, setFromLang] = useState('auto');
  const [toLang, setToLang] = useState('zh');

  // OCR state
  const [ocrRunning, setOcrRunning] = useState(false);
  const [ocrProgress, setOcrProgress] = useState<OcrProgress | null>(null);
  const [ocrLang, setOcrLang] = useState<string>('auto');

  const { t } = useI18n();
  const isTauri = isTauriRuntime();

  // Listen for OCR progress events
  useEffect(() => {
    if (!isTauri) return;

    let unlisten: (() => void) | undefined;

    listen<OcrProgress>('pdf-ocr-progress', (event) => {
      setOcrProgress(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, [isTauri]);

  const openFile = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'PDF',
            extensions: ['pdf'],
          },
        ],
      });

      if (selected) {
        const path = selected;
        setFilePath(path);
        setFileName(path.split(/[/\\]/).pop() || 'document.pdf');
        setTranslatedPdf(null);
        setCurrentPage(1);
        setOcrProgress(null);

        // Load PDF content
        setLoading(true);
        try {
          const doc = await invokeOrThrow<PdfDocument>('open_pdf', { filePath: path });
          setPdfDoc(doc);
        } catch (err) {
          console.error('Failed to open PDF:', err);
        } finally {
          setLoading(false);
        }
      }
    } catch (err) {
      console.error('Failed to open file dialog:', err);
    }
  };

  const translatePdf = async () => {
    if (!filePath) return;

    setTranslating(true);
    try {
      const result = await invokeOrThrow<TranslatedPdf>('translate_pdf', {
        filePath,
        fromLang,
        toLang,
      });
      setTranslatedPdf(result);
    } catch (err) {
      console.error('Failed to translate PDF:', err);
    } finally {
      setTranslating(false);
    }
  };

  const runOcr = async () => {
    if (!filePath) return;

    setOcrRunning(true);
    setOcrProgress(null);
    try {
      const result = await invokeOrThrow<ScannedPdfOcrResult>('ocr_scanned_pdf', {
        filePath,
        lang: ocrLang === 'auto' ? null : ocrLang,
      });

      // Update pdfDoc with OCR results
      setPdfDoc({
        pages: result.pages,
        totalPages: result.totalPages,
        isScanned: true,
      });
      setCurrentPage(1);
    } catch (err) {
      console.error('Failed to OCR PDF:', err);
    } finally {
      setOcrRunning(false);
      setOcrProgress(null);
    }
  };

  const exportTranslatedPdf = () => {
    if (!translatedPdf) return;

    let content = '';
    for (const page of translatedPdf.pages) {
      content += `=== Page ${page.pageNumber} ===\n\n`;
      content += `--- Original ---\n${page.originalText}\n\n`;
      content += `--- Translation ---\n${page.translatedText}\n\n`;
      content += '\n';
    }

    const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${fileName.replace('.pdf', '')}_translated.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const currentPdfPage = pdfDoc?.pages.find((p) => p.pageNumber === currentPage);
  const currentTranslatedPage = translatedPdf?.pages.find((p) => p.pageNumber === currentPage);
  const hasOcrText = pdfDoc?.pages.some((p) => p.text.trim().length > 0) ?? false;

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header */}
      <div className="flex justify-between items-center mb-5">
        <h1 className="text-2xl font-bold">{t('pdf.title')}</h1>
        <div className="flex items-center gap-3">
          <select
            value={fromLang}
            onChange={(e) => setFromLang(e.target.value)}
            className="bg-bg-secondary text-text-primary border border-border rounded-lg px-3 py-2 text-sm cursor-pointer focus:border-primary"
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
            className="bg-bg-secondary text-text-primary border border-border rounded-lg px-3 py-2 text-sm cursor-pointer focus:border-primary"
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
            <option value="ja">日本語</option>
            <option value="ko">한국어</option>
          </select>
          <button
            className="bg-primary text-primary-fg border border-primary rounded-lg px-4 py-2 text-sm hover:bg-primary/80 transition-colors flex items-center gap-1.5"
            onClick={openFile}
          >
            <FileText size={14} />
            {t('pdf.openFile')}
          </button>
          {pdfDoc && !pdfDoc.isScanned && (
            <button
              className="bg-accent text-white border border-accent rounded-lg px-4 py-2 text-sm hover:bg-accent/80 transition-colors flex items-center gap-1.5 disabled:opacity-50"
              onClick={translatePdf}
              disabled={translating}
            >
              <Languages size={14} />
              {translating ? t('pdf.translating') : t('pdf.translate')}
            </button>
          )}
          {pdfDoc && hasOcrText && pdfDoc.isScanned && (
            <button
              className="bg-accent text-white border border-accent rounded-lg px-4 py-2 text-sm hover:bg-accent/80 transition-colors flex items-center gap-1.5 disabled:opacity-50"
              onClick={translatePdf}
              disabled={translating}
            >
              <Languages size={14} />
              {translating ? t('pdf.translating') : t('pdf.translate')}
            </button>
          )}
          {translatedPdf && (
            <>
              <button
                className={`border rounded-lg px-4 py-2 text-sm transition-colors ${
                  showBilingual
                    ? 'bg-primary text-primary-fg border-primary'
                    : 'bg-bg-tertiary text-text-secondary border-border hover:bg-bg-tertiary/80'
                }`}
                onClick={() => setShowBilingual(!showBilingual)}
              >
                {t('pdf.bilingual')}
              </button>
              <button
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-primary-fg hover:border-primary transition-colors flex items-center gap-1.5"
                onClick={exportTranslatedPdf}
              >
                <Download size={14} />
                {t('pdf.export')}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {!filePath ? (
          <div className="flex flex-col items-center justify-center h-full text-text-secondary">
            <FileText size={64} className="mb-4 opacity-50" />
            <p className="text-lg mb-2">{t('pdf.noFile')}</p>
            <p className="text-sm">{t('pdf.openHint')}</p>
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            <div className="animate-pulse">{t('pdf.loading')}</div>
          </div>
        ) : pdfDoc ? (
          <div className="h-full flex flex-col">
            {/* Scanned PDF Detection Banner */}
            {pdfDoc.isScanned && !hasOcrText && (
              <div className="mb-4 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <ScanLine size={18} className="text-amber-500" />
                    <div>
                      <p className="text-sm font-medium text-amber-400">
                        {t('pdf.scannedDetected')}
                      </p>
                      <p className="text-xs text-text-secondary mt-1">
                        {t('pdf.scannedHint', { total: String(pdfDoc.totalPages) })}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <select
                      value={ocrLang}
                      onChange={(e) => setOcrLang(e.target.value)}
                      className="bg-bg-secondary text-text-primary border border-border rounded-lg px-2 py-1.5 text-xs cursor-pointer"
                    >
                      <option value="auto">Auto</option>
                      <option value="en">English</option>
                      <option value="zh-Hans">中文</option>
                      <option value="ja">日本語</option>
                      <option value="ko">한국어</option>
                      <option value="de">Deutsch</option>
                      <option value="fr">Français</option>
                    </select>
                    <button
                      className="bg-amber-600 text-white border border-amber-600 rounded-lg px-4 py-2 text-sm hover:bg-amber-700 transition-colors flex items-center gap-1.5 disabled:opacity-50"
                      onClick={runOcr}
                      disabled={ocrRunning}
                    >
                      {ocrRunning ? (
                        <>
                          <Loader2 size={14} className="animate-spin" />
                          {t('pdf.ocrRunning')}
                        </>
                      ) : (
                        <>
                          <ScanLine size={14} />
                          {t('pdf.ocrButton')}
                        </>
                      )}
                    </button>
                  </div>
                </div>
                {/* OCR Progress Bar */}
                {ocrProgress && (
                  <div className="mt-3">
                    <div className="flex items-center justify-between text-xs text-text-secondary mb-1">
                      <span>
                        {t('pdf.ocrProgress', {
                          current: String(ocrProgress.current),
                          total: String(ocrProgress.total),
                        })}
                      </span>
                      <span>{Math.round((ocrProgress.current / ocrProgress.total) * 100)}%</span>
                    </div>
                    <div className="w-full bg-bg-tertiary rounded-full h-2">
                      <div
                        className="bg-amber-500 h-2 rounded-full transition-all duration-300"
                        style={{ width: `${(ocrProgress.current / ocrProgress.total) * 100}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* Page Info */}
            <div className="flex items-center justify-between mb-4">
              <span className="text-sm text-text-secondary">
                {t('pdf.pageInfo', {
                  current: String(currentPage),
                  total: String(pdfDoc.totalPages),
                })}
                {pdfDoc.isScanned && hasOcrText && (
                  <span className="ml-2 text-amber-500 text-xs">(OCR)</span>
                )}
              </span>
              <div className="flex items-center gap-2">
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={currentPage <= 1}
                >
                  <ChevronLeft size={16} />
                </button>
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentPage((p) => Math.min(pdfDoc.totalPages, p + 1))}
                  disabled={currentPage >= pdfDoc.totalPages}
                >
                  <ChevronRight size={16} />
                </button>
              </div>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-y-auto">
              {showBilingual && translatedPdf ? (
                /* Bilingual View */
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-bg-secondary border border-border rounded-xl p-4">
                    <h3 className="text-xs font-semibold text-text-secondary uppercase mb-3">
                      {t('pdf.original')}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap">
                      {currentPdfPage?.text || t('pdf.emptyPage')}
                    </div>
                  </div>
                  <div className="bg-bg-secondary border border-border rounded-xl p-4">
                    <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                      {t('pdf.translation')}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap text-primary">
                      {currentTranslatedPage?.translatedText || t('pdf.notTranslated')}
                    </div>
                  </div>
                </div>
              ) : translatedPdf ? (
                /* Translation Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                    {t('pdf.translation')}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentTranslatedPage?.translatedText || t('pdf.notTranslated')}
                  </div>
                </div>
              ) : (
                /* Original Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-text-secondary uppercase mb-3">
                    {t('pdf.original')}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentPdfPage?.text || t('pdf.emptyPage')}
                  </div>
                </div>
              )}
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

export default PdfViewer;
