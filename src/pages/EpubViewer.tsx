import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { BookOpen, Languages, Download, ChevronLeft, ChevronRight } from "lucide-react";

interface EpubChapter {
  chapterNumber: number;
  title: string;
  text: string;
}

interface TranslatedChapter {
  chapterNumber: number;
  title: string;
  originalText: string;
  translatedText: string;
}

interface EpubDocument {
  title: string;
  chapters: EpubChapter[];
  totalChapters: number;
}

interface TranslatedEpub {
  title: string;
  chapters: TranslatedChapter[];
  totalChapters: number;
}

function EpubViewer() {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string>("");
  const [epubDoc, setEpubDoc] = useState<EpubDocument | null>(null);
  const [translatedEpub, setTranslatedEpub] = useState<TranslatedEpub | null>(null);
  const [currentChapter, setCurrentChapter] = useState(1);
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
            name: "EPUB",
            extensions: ["epub"],
          },
        ],
      });

      if (selected) {
        const path = typeof selected === "string" ? selected : (selected as any).path;
        setFilePath(path);
        setFileName(path.split(/[/\\]/).pop() || "book.epub");
        setTranslatedEpub(null);
        setCurrentChapter(1);

        // Load EPUB content
        setLoading(true);
        try {
          const doc = await invoke<EpubDocument>("open_epub", { filePath: path });
          setEpubDoc(doc);
        } catch (err) {
          console.error("Failed to open EPUB:", err);
        } finally {
          setLoading(false);
        }
      }
    } catch (err) {
      console.error("Failed to open file dialog:", err);
    }
  };

  const translateEpub = async () => {
    if (!filePath) return;

    setTranslating(true);
    try {
      const result = await invoke<TranslatedEpub>("translate_epub", {
        filePath,
        fromLang,
        toLang,
      });
      setTranslatedEpub(result);
    } catch (err) {
      console.error("Failed to translate EPUB:", err);
    } finally {
      setTranslating(false);
    }
  };

  const exportTranslatedEpub = () => {
    if (!translatedEpub) return;

    let content = `# ${translatedEpub.title}\n\n`;
    for (const chapter of translatedEpub.chapters) {
      content += `## ${chapter.title}\n\n`;
      content += `### Original\n${chapter.originalText}\n\n`;
      content += `### Translation\n${chapter.translatedText}\n\n`;
      content += "---\n\n";
    }

    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${fileName.replace(".epub", "")}_translated.txt`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const currentEpubChapter = epubDoc?.chapters.find((c) => c.chapterNumber === currentChapter);
  const currentTranslatedChapter = translatedEpub?.chapters.find((c) => c.chapterNumber === currentChapter);

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header */}
      <div className="flex justify-between items-center mb-5">
        <h1 className="text-2xl font-bold">{t("epub.title")}</h1>
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
            <BookOpen size={14} />
            {t("epub.openFile")}
          </button>
          {epubDoc && (
            <button
              className="bg-accent text-white border border-accent rounded-lg px-4 py-2 text-sm hover:bg-accent/80 transition-colors flex items-center gap-1.5 disabled:opacity-50"
              onClick={translateEpub}
              disabled={translating}
            >
              <Languages size={14} />
              {translating ? t("epub.translating") : t("epub.translate")}
            </button>
          )}
          {translatedEpub && (
            <>
              <button
                className={`border rounded-lg px-4 py-2 text-sm transition-colors ${
                  showBilingual
                    ? "bg-primary text-white border-primary"
                    : "bg-bg-tertiary text-text-secondary border-border hover:bg-bg-tertiary/80"
                }`}
                onClick={() => setShowBilingual(!showBilingual)}
              >
                {t("epub.bilingual")}
              </button>
              <button
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1.5"
                onClick={exportTranslatedEpub}
              >
                <Download size={14} />
                {t("epub.export")}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {!filePath ? (
          <div className="flex flex-col items-center justify-center h-full text-text-secondary">
            <BookOpen size={64} className="mb-4 opacity-50" />
            <p className="text-lg mb-2">{t("epub.noFile")}</p>
            <p className="text-sm">{t("epub.openHint")}</p>
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            <div className="animate-pulse">{t("epub.loading")}</div>
          </div>
        ) : epubDoc ? (
          <div className="h-full flex flex-col">
            {/* Book Info */}
            <div className="mb-4">
              <h2 className="text-lg font-semibold text-text-primary">{epubDoc.title}</h2>
            </div>

            {/* Chapter Navigation */}
            <div className="flex items-center justify-between mb-4">
              <span className="text-sm text-text-secondary">
                {t("epub.chapterInfo", {
                  current: String(currentChapter),
                  total: String(epubDoc.totalChapters),
                })}
              </span>
              <div className="flex items-center gap-2">
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentChapter((c) => Math.max(1, c - 1))}
                  disabled={currentChapter <= 1}
                >
                  <ChevronLeft size={16} />
                </button>
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentChapter((c) => Math.min(epubDoc.totalChapters, c + 1))}
                  disabled={currentChapter >= epubDoc.totalChapters}
                >
                  <ChevronRight size={16} />
                </button>
              </div>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-y-auto">
              {showBilingual && translatedEpub ? (
                /* Bilingual View */
                <div className="grid grid-cols-2 gap-4">
                  <div className="bg-bg-secondary border border-border rounded-xl p-4">
                    <h3 className="text-xs font-semibold text-text-secondary uppercase mb-3">
                      {t("epub.original")}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap">
                      {currentEpubChapter?.text || t("epub.emptyChapter")}
                    </div>
                  </div>
                  <div className="bg-bg-secondary border border-border rounded-xl p-4">
                    <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                      {t("epub.translation")}
                    </h3>
                    <div className="text-sm leading-relaxed whitespace-pre-wrap text-primary">
                      {currentTranslatedChapter?.translatedText || t("epub.notTranslated")}
                    </div>
                  </div>
                </div>
              ) : translatedEpub ? (
                /* Translation Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-primary uppercase mb-3">
                    {t("epub.translation")}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentTranslatedChapter?.translatedText || t("epub.notTranslated")}
                  </div>
                </div>
              ) : (
                /* Original Only View */
                <div className="bg-bg-secondary border border-border rounded-xl p-4">
                  <h3 className="text-xs font-semibold text-text-secondary uppercase mb-3">
                    {t("epub.original")}
                  </h3>
                  <div className="text-sm leading-relaxed whitespace-pre-wrap">
                    {currentEpubChapter?.text || t("epub.emptyChapter")}
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

export default EpubViewer;
