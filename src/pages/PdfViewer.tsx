import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { FileText, Languages, Download, ChevronLeft, ChevronRight } from "lucide-react";

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
}

interface TranslatedPdf {
  pages: TranslatedPage[];
  totalPages: number;
}

function PdfViewer() {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string>("");
  const [pdfDoc, setPdfDoc] = useState<PdfDocument | null>(null);
  const [translatedPdf, setTranslatedPdf] = useState<TranslatedPdf | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [showBilingual, setShowBilingual] = useState(true);
  const [fromLang, setFromLang] = useState("auto");
  const [toLang, setToLang] = useState("zh");
  const { t } = useI18n();

  const openFile = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "PDF",
            extensions: ["pdf"],
          },
        ],
      });

      if (selected) {
        const path = typeof selected === "string" ? selected : (selected as any).path;
        setFilePath(path);
        setFileName(path.split(/[/\\]/).pop() || "document.pdf");
        setTranslatedPdf(null);
        setCurrentPage(1);

        // Load PDF content
        setLoading(true);
        try {
          const doc = await invoke<PdfDocument>("open_pdf", { filePath: path });
          setPdfDoc(doc);
        } catch (err) {
          console.error("Failed to open PDF:", err);
        } finally {
          setLoading(false);
        }
      }
    } catch (err) {
      console.error("Failed to open file dialog:", err);
    }
  };

  const translatePdf = async () => {
    if (!filePath) return;

    setTranslating(true);
    try {
      const result = await invoke<TranslatedPdf>("translate_pdf", {
        filePath,
        fromLang,
        toLang,
      });
      setTranslatedPdf(result);
    } catch (err) {
      console.error("Failed to translate PDF:", err);
    } finally {
      setTranslating(false);
    }
  };

  const exportTranslatedPdf = () => {
    if (!translatedPdf) return;

    let content = "";
    for (const page of translatedPdf.pages) {
      content += `=== Page ${page.pageNumber} ===\n\n`;
      content += `--- Original ---\n${page.originalText}\n\n`;
      content += `--- Translation ---\n${page.translatedText}\n\n`;
      content += "\n";
    }

    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${fileName.replace(".pdf", "")}_translated.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const currentPdfPage = pdfDoc?.pages.find((p) => p.pageNumber === currentPage);
  const currentTranslatedPage = translatedPdf?.pages.find((p) => p.pageNumber === currentPage);

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header */}
      <div className="flex justify-between items-center mb-5">
        <h1 className="text-2xl font-bold">{t("pdf.title")}</h1>
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
            className="bg-primary text-white border border-primary rounded-lg px-4 py-2 text-sm hover:bg-primary/80 transition-colors flex items-center gap-1.5"
            onClick={openFile}
          >
            <FileText size={14} />
            {t("pdf.openFile")}
          </button>
          {pdfDoc && (
            <button
              className="bg-accent text-white border border-accent rounded-lg px-4 py-2 text-sm hover:bg-accent/80 transition-colors flex items-center gap-1.5 disabled:opacity-50"
              onClick={translatePdf}
              disabled={translating}
            >
              <Languages size={14} />
              {translating ? t("pdf.translating") : t("pdf.translate")}
            </button>
          )}
          {translatedPdf && (
            <>
              <button
                className={`border rounded-lg px-4 py-2 text-sm transition-colors ${
                  showBilingual
                    ? "bg-primary text-white border-primary"
                    : "bg-bg-tertiary text-text-secondary border-border hover:bg-bg-tertiary/80"
                }`}
                onClick={() => setShowBilingual(!showBilingual)}
              >
                {t("pdf.bilingual")}
              </button>
              <button
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1.5"
                onClick={exportTranslatedPdf}
              >
                <Download size={14} />
                {t("pdf.export")}
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
            <p className="text-lg mb-2">{t("pdf.noFile")}</p>
            <p className="text-sm">{t("pdf.openHint")}</p>
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            <div className="animate-pulse">{t("pdf.loading")}</div>
          </div>
        ) : pdfDoc ? (
          <div className="h-full flex flex-col">
            {/* Page Info */}
            <div className="flex items-center justify-between mb-4">
              <span className="text-sm text-text-secondary">
                {t("pdf.pageInfo", {
                  current: String(currentPage),
                  total: String(pdfDoc.totalPages),
                })}
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
                      {t("pdf.original")}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap">
                      {currentPdfPage?.text || t("pdf.emptyPage")}
                    </div>
                  </div>
                  <div className="bg-bg-secondary border border-border rounded-xl p-4">
                    <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                      {t("pdf.translation")}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap text-primary">
                      {currentTranslatedPage?.translatedText || t("pdf.notTranslated")}
                    </div>
                  </div>
                </div>
              ) : translatedPdf ? (
                /* Translation Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                    {t("pdf.translation")}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentTranslatedPage?.translatedText || t("pdf.notTranslated")}
                  </div>
                </div>
              ) : (
                /* Original Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-text-secondary uppercase mb-3">
                    {t("pdf.original")}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentPdfPage?.text || t("pdf.emptyPage")}
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
