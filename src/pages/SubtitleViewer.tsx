import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { FileText, Languages, Download, ChevronLeft, ChevronRight, Subtitles, Loader2 } from "lucide-react";

interface SubtitleEntry {
  index: number;
  startTime: string;
  endTime: string;
  originalText: string;
  translatedText: string;
}

interface SubtitleDocument {
  entries: SubtitleEntry[];
  totalEntries: number;
  format: string;
}

interface TranslatedSubtitle {
  entries: SubtitleEntry[];
  totalEntries: number;
  format: string;
}

interface ProgressInfo {
  current: number;
  total: number;
  text: string;
}

function SubtitleViewer() {
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string>("");
  const [subtitleDoc, setSubtitleDoc] = useState<SubtitleDocument | null>(null);
  const [translatedSub, setTranslatedSub] = useState<TranslatedSubtitle | null>(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [loading, setLoading] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [progress, setProgress] = useState<ProgressInfo | null>(null);
  const [showBilingual, setShowBilingual] = useState(true);
  const [fromLang, setFromLang] = useState("auto");
  const [toLang, setToLang] = useState("zh");
  const [itemsPerPage] = useState(20);
  const { t } = useI18n();

  // Listen for progress events
  useEffect(() => {
    const unlisten = listen<ProgressInfo>("subtitle-progress", (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const openFile = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Subtitle",
            extensions: ["srt", "ass", "ssa", "vtt", "lrc"],
          },
        ],
      });

      if (selected) {
        const path = typeof selected === "string" ? selected : (selected as any).path;
        setFilePath(path);
        setFileName(path.split(/[/\\]/).pop() || "subtitle.srt");
        setTranslatedSub(null);
        setCurrentPage(1);
        setProgress(null);

        // Load subtitle content
        setLoading(true);
        try {
          const doc = await invoke<SubtitleDocument>("open_subtitle", { filePath: path });
          setSubtitleDoc(doc);
        } catch (err) {
          console.error("Failed to open subtitle:", err);
        } finally {
          setLoading(false);
        }
      }
    } catch (err) {
      console.error("Failed to open file dialog:", err);
    }
  };

  const translateSubtitle = async () => {
    if (!filePath) return;

    setTranslating(true);
    setProgress(null);
    try {
      const result = await invoke<TranslatedSubtitle>("translate_subtitle", {
        filePath,
        fromLang,
        toLang,
      });
      setTranslatedSub(result);
    } catch (err) {
      console.error("Failed to translate subtitle:", err);
    } finally {
      setTranslating(false);
      setProgress(null);
    }
  };

  const exportTranslated = async () => {
    if (!translatedSub) return;

    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const ext = subtitleDoc?.format || "srt";
      const defaultName = fileName.replace(/\.[^.]+$/, `_translated.${ext}`);

      const outputPath = await save({
        defaultPath: defaultName,
        filters: [
          {
            name: "Subtitle",
            extensions: [ext],
          },
        ],
      });

      if (outputPath) {
        // Generate export content
        let content = "";
        if (ext === "srt" || ext === "vtt") {
          for (const entry of translatedSub.entries) {
            if (ext === "srt") {
              content += `${entry.index}\n`;
              content += `${entry.startTime} --> ${entry.endTime}\n`;
              content += `${showBilingual ? entry.originalText + "\n" : ""}${entry.translatedText}\n\n`;
            } else {
              content += `${entry.index}\n`;
              content += `${entry.startTime} --> ${entry.endTime}\n`;
              content += `${showBilingual ? entry.originalText + "\n" : ""}${entry.translatedText}\n\n`;
            }
          }
          if (ext === "vtt") {
            content = "WEBVTT\n\n" + content;
          }
        } else if (ext === "lrc") {
          for (const entry of translatedSub.entries) {
            content += `${entry.startTime}${entry.originalText}\n`;
            content += `${entry.startTime}[译] ${entry.translatedText}\n`;
          }
        }

        // Write file using invoke
        await invoke("export_subtitle_file", {
          filePath: filePath!,
          outputPath,
          bilingual: showBilingual,
        });
      }
    } catch (err) {
      console.error("Failed to export:", err);
    }
  };

  // Calculate pagination
  const totalEntries = translatedSub?.totalEntries || subtitleDoc?.totalEntries || 0;
  const totalPages = Math.ceil(totalEntries / itemsPerPage);
  const startIdx = (currentPage - 1) * itemsPerPage;
  const endIdx = Math.min(startIdx + itemsPerPage, totalEntries);

  const currentEntries = (translatedSub?.entries || subtitleDoc?.entries || []).slice(startIdx, endIdx);

  return (
    <div className="h-full flex flex-col p-6">
      {/* Header */}
      <div className="flex justify-between items-center mb-5">
        <h1 className="text-2xl font-bold flex items-center gap-2">
          <Subtitles size={24} />
          {t("subtitle.title")}
        </h1>
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
            {t("subtitle.openFile")}
          </button>
          {subtitleDoc && (
            <button
              className="bg-accent text-white border border-accent rounded-lg px-4 py-2 text-sm hover:bg-accent/80 transition-colors flex items-center gap-1.5 disabled:opacity-50"
              onClick={translateSubtitle}
              disabled={translating}
            >
              {translating ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <Languages size={14} />
              )}
              {translating ? t("subtitle.translating") : t("subtitle.translate")}
            </button>
          )}
          {translatedSub && (
            <>
              <button
                className={`border rounded-lg px-4 py-2 text-sm transition-colors ${
                  showBilingual
                    ? "bg-primary text-white border-primary"
                    : "bg-bg-tertiary text-text-secondary border-border hover:bg-bg-tertiary/80"
                }`}
                onClick={() => setShowBilingual(!showBilingual)}
              >
                {t("subtitle.bilingual")}
              </button>
              <button
                className="bg-bg-tertiary text-text-secondary border border-border rounded-lg px-4 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center gap-1.5"
                onClick={exportTranslated}
              >
                <Download size={14} />
                {t("subtitle.export")}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Progress Bar */}
      {translating && progress && (
        <div className="mb-4">
          <div className="flex justify-between text-sm text-text-secondary mb-1">
            <span>{t("subtitle.translating")}</span>
            <span>{progress.current} / {progress.total}</span>
          </div>
          <div className="w-full bg-bg-tertiary rounded-full h-2">
            <div
              className="bg-primary h-2 rounded-full transition-all duration-300"
              style={{ width: `${(progress.current / progress.total) * 100}%` }}
            />
          </div>
          <p className="text-xs text-text-secondary mt-1 truncate">{progress.text}</p>
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {!filePath ? (
          <div className="flex flex-col items-center justify-center h-full text-text-secondary">
            <Subtitles size={64} className="mb-4 opacity-50" />
            <p className="text-lg mb-2">{t("subtitle.noFile")}</p>
            <p className="text-sm">{t("subtitle.openHint")}</p>
            <p className="text-sm mt-2">{t("subtitle.supportedFormats")}</p>
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center h-full text-text-secondary">
            <div className="animate-pulse">{t("subtitle.loading")}</div>
          </div>
        ) : subtitleDoc ? (
          <div className="h-full flex flex-col">
            {/* Entry Info & Pagination */}
            <div className="flex items-center justify-between mb-4">
              <span className="text-sm text-text-secondary">
                {t("subtitle.entryInfo", {
                  current: String(currentPage),
                  total: String(totalPages),
                  count: String(totalEntries),
                })}
                {" "}
                <span className="bg-bg-tertiary px-2 py-0.5 rounded text-xs uppercase">
                  {subtitleDoc.format}
                </span>
              </span>
              <div className="flex items-center gap-2">
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
                  disabled={currentPage <= 1}
                >
                  <ChevronLeft size={16} />
                </button>
                <span className="text-sm text-text-secondary">
                  {currentPage} / {totalPages}
                </span>
                <button
                  className="p-2 rounded-lg bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80 disabled:opacity-50"
                  onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
                  disabled={currentPage >= totalPages}
                >
                  <ChevronRight size={16} />
                </button>
              </div>
            </div>

            {/* Subtitle Table */}
            <div className="flex-1 overflow-y-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border">
                    <th className="text-left py-2 px-3 text-text-secondary font-medium w-16">#</th>
                    <th className="text-left py-2 px-3 text-text-secondary font-medium w-32">{t("subtitle.time")}</th>
                    <th className="text-left py-2 px-3 text-text-secondary font-medium">{t("subtitle.original")}</th>
                    {translatedSub && (
                      <th className="text-left py-2 px-3 text-primary font-medium">{t("subtitle.translation")}</th>
                    )}
                  </tr>
                </thead>
                <tbody>
                  {currentEntries.map((entry) => (
                    <tr key={entry.index} className="border-b border-border/50 hover:bg-bg-secondary/50">
                      <td className="py-2 px-3 text-text-secondary">{entry.index}</td>
                      <td className="py-2 px-3 text-text-secondary text-xs font-mono">
                        {entry.startTime}
                        {entry.endTime && ` → ${entry.endTime}`}
                      </td>
                      <td className="py-2 px-3">{entry.originalText}</td>
                      {translatedSub && (
                        <td className="py-2 px-3 text-primary">
                          {entry.translatedText || <span className="text-text-secondary italic">{t("subtitle.notTranslated")}</span>}
                        </td>
                      )}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

export default SubtitleViewer;
