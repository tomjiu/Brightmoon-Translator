import { useState, useEffect, useCallback } from "react";
import { invokeOrThrow } from "../services/invoke";
import { useI18n } from "../i18n";
import { useToastStore } from "../stores/toastStore";
import { isTauriRuntime } from "../services/tauriRuntime";
import { LANGUAGES, type TmStats, type TmExportEntry } from "../types";
import {
  Database,
  Download,
  Upload,
  RefreshCw,
  FileJson,
  FileText,
  BarChart3,
  Search,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";

const PAGE_SIZE = 20;

function TmManager() {
  const { t } = useI18n();
  const addToast = useToastStore((state) => state.addToast);
  const isTauri = isTauriRuntime();
  const [stats, setStats] = useState<TmStats | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exporting, setExporting] = useState(false);

  // Search state
  const [searchQuery, setSearchQuery] = useState("");
  const [searchFromLang, setSearchFromLang] = useState<string>("");
  const [searchToLang, setSearchToLang] = useState<string>("");
  const [searchResults, setSearchResults] = useState<TmExportEntry[]>([]);
  const [searchTotal, setSearchTotal] = useState(0);
  const [searchPage, setSearchPage] = useState(0);
  const [searching, setSearching] = useState(false);

  const loadStats = useCallback(async () => {
    if (!isTauri) return;

    setLoading(true);
    try {
      const result = await invokeOrThrow<TmStats>("tm_get_stats");
      setStats(result);
    } catch (err) {
      console.error("Failed to load TM stats:", err);
    } finally {
      setLoading(false);
    }
  }, [isTauri]);

  useEffect(() => {
    if (!isTauri) return;

    loadStats();
  }, [isTauri, loadStats]);

  const handleSearch = useCallback(async (page = 0) => {
    if (!isTauri) return;

    if (!searchQuery.trim() && !searchFromLang && !searchToLang) {
      setSearchResults([]);
      setSearchTotal(0);
      return;
    }

    setSearching(true);
    try {
      const [entries, total] = await invokeOrThrow<[TmExportEntry[], number]>("tm_search", {
        query: searchQuery.trim(),
        fromLang: searchFromLang || null,
        toLang: searchToLang || null,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      });
      setSearchResults(entries);
      setSearchTotal(total);
      setSearchPage(page);
    } catch (err) {
      console.error("TM search failed:", err);
    } finally {
      setSearching(false);
    }
  }, [isTauri, searchQuery, searchFromLang, searchToLang]);

  const totalPages = Math.ceil(searchTotal / PAGE_SIZE);

  const handleExport = useCallback(async () => {
    setExporting(true);
    try {
      const json = await invokeOrThrow<string>("tm_export", {
        fromLang: null,
        toLang: null,
      });

      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `translation-memory-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error("Failed to export TM:", err);
    } finally {
      setExporting(false);
    }
  }, []);

  const handleExportTmx = useCallback(async () => {
    setExporting(true);
    try {
      const xml = await invokeOrThrow<string>("tm_export_tmx", {
        fromLang: null,
        toLang: null,
      });

      const blob = new Blob([xml], { type: "application/xml" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `translation-memory-${new Date().toISOString().slice(0, 10)}.tmx`;
      a.click();
      URL.revokeObjectURL(url);
      addToast({
        type: "success",
        message: t("tm.tmxExportSuccess") || "TMX 导出成功",
        duration: 3000,
      });
    } catch (err) {
      console.error("Failed to export TMX:", err);
      addToast({
        type: "error",
        message: `${t("tm.tmxExportFailed") || "TMX 导出失败"}: ${err}`,
        duration: 4000,
      });
    } finally {
      setExporting(false);
    }
  }, [addToast]);

  const handleImport = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";

    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setImporting(true);
      try {
        const text = await file.text();
        const result = await invokeOrThrow<[number, number]>("tm_import", {
          json: text,
          deduplicate: true,
        });

        // Reload stats after import
        await loadStats();
        addToast({
          type: "success",
          message: `${t("tm.importSuccess") || "导入成功"}: ${result[0]} ${t("tm.records") || "条记录"}, ${result[1]} ${t("tm.duplicatesSkipped") || "条重复跳过"}`,
          duration: 3000,
        });
      } catch (err) {
        console.error("Failed to import TM:", err);
        addToast({
          type: "error",
          message: `${t("tm.importFailed") || "导入失败"}: ${err}`,
          duration: 4000,
        });
      } finally {
        setImporting(false);
      }
    };

    input.click();
  }, [loadStats, addToast]);

  const handleImportTmx = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".tmx,.xml";

    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (!file) return;

      setImporting(true);
      try {
        const text = await file.text();
        const result = await invokeOrThrow<[number, number]>("tm_import_tmx", {
          xml: text,
          deduplicate: true,
        });

        // Reload stats after import
        await loadStats();
        addToast({
          type: "success",
          message: `${t("tm.tmxImportSuccess") || "TMX 导入成功"}: ${result[0]} ${t("tm.records") || "条记录"}, ${result[1]} ${t("tm.duplicatesSkipped") || "条重复跳过"}`,
          duration: 3000,
        });
      } catch (err) {
        console.error("Failed to import TMX:", err);
        addToast({
          type: "error",
          message: `${t("tm.tmxImportFailed") || "TMX 导入失败"}: ${err}`,
          duration: 4000,
        });
      } finally {
        setImporting(false);
      }
    };

    input.click();
  }, [loadStats, addToast]);

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Database size={18} className="text-primary" />
          <h3 className="text-sm font-semibold text-text-primary">
            {t("tm.title")}
          </h3>
        </div>
        <button
          className="p-1.5 rounded-lg hover:bg-bg-tertiary text-text-secondary transition-colors"
          onClick={loadStats}
          disabled={loading}
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {/* Statistics */}
      {stats && (
        <div className="bg-bg-tertiary rounded-lg p-4 space-y-3">
          <div className="flex items-center gap-2 text-text-secondary">
            <BarChart3 size={14} />
            <span className="text-xs font-medium">{t("tm.statistics")}</span>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div className="bg-bg-secondary rounded-lg p-3">
              <p className="text-xs text-text-secondary">{t("tm.totalEntries")}</p>
              <p className="text-lg font-semibold text-text-primary">
                {stats.total.toLocaleString()}
              </p>
            </div>
            <div className="bg-bg-secondary rounded-lg p-3">
              <p className="text-xs text-text-secondary">{t("tm.languagePairs")}</p>
              <p className="text-lg font-semibold text-text-primary">
                {stats.langPairs.length}
              </p>
            </div>
          </div>

          {/* Language Pair Breakdown */}
          {stats.langPairs.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs text-text-secondary font-medium">{t("tm.languagePairs")}:</p>
              <div className="max-h-32 overflow-y-auto space-y-1">
                {stats.langPairs.map(([from, to, count], idx) => (
                  <div
                    key={idx}
                    className="flex items-center justify-between bg-bg-secondary rounded px-3 py-1.5 text-xs"
                  >
                    <span className="text-text-primary font-mono">
                      {from} → {to}
                    </span>
                    <span className="text-text-secondary">
                      {count.toLocaleString()} {t("tm.entries")}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Actions - JSON */}
      <div className="flex gap-2">
        <button
          className="flex-1 bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center justify-center gap-2"
          onClick={handleExport}
          disabled={exporting || !stats || stats.total === 0}
        >
          <Download size={14} />
          <FileJson size={14} />
          {exporting ? t("tm.exporting") : "JSON"}
        </button>
        <button
          className="flex-1 bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center justify-center gap-2"
          onClick={handleImport}
          disabled={importing}
        >
          <Upload size={14} />
          <FileJson size={14} />
          {importing ? t("tm.importing") : "JSON"}
        </button>
      </div>

      {/* Actions - TMX */}
      <div className="flex gap-2">
        <button
          className="flex-1 bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-primary hover:text-white hover:border-primary transition-colors flex items-center justify-center gap-2"
          onClick={handleExportTmx}
          disabled={exporting || !stats || stats.total === 0}
        >
          <Download size={14} />
          <FileText size={14} />
          {exporting ? t("tm.exporting") : "TMX"}
        </button>
        <button
          className="flex-1 bg-bg-tertiary text-text-secondary border border-border rounded-lg px-3 py-2 text-sm hover:bg-accent hover:text-white hover:border-accent transition-colors flex items-center justify-center gap-2"
          onClick={handleImportTmx}
          disabled={importing}
        >
          <Upload size={14} />
          <FileText size={14} />
          {importing ? t("tm.importing") : "TMX"}
        </button>
      </div>

      {/* Search Section */}
      <div className="space-y-3">
        <div className="flex items-center gap-2 text-text-secondary">
          <Search size={14} />
          <span className="text-xs font-medium">{t("tm.searchTitle")}</span>
        </div>

        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("tm.searchPlaceholder")}
            className="flex-1 bg-bg-tertiary border border-border rounded-lg px-3 py-2 text-sm text-text-primary outline-none focus:border-primary"
            onKeyDown={(e) => e.key === "Enter" && handleSearch(0)}
          />
          <button
            className="px-3 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-hover transition-colors"
            onClick={() => handleSearch(0)}
            disabled={searching}
          >
            <Search size={14} />
          </button>
        </div>

        <div className="flex gap-2">
          <select
            value={searchFromLang}
            onChange={(e) => setSearchFromLang(e.target.value)}
            className="flex-1 bg-bg-tertiary border border-border rounded px-2 py-1.5 text-xs text-text-primary outline-none"
          >
            <option value="">{t("tm.anySourceLang")}</option>
            {LANGUAGES.filter((l) => l.code !== "auto").map((lang) => (
              <option key={lang.code} value={lang.code}>
                {lang.name}
              </option>
            ))}
          </select>
          <select
            value={searchToLang}
            onChange={(e) => setSearchToLang(e.target.value)}
            className="flex-1 bg-bg-tertiary border border-border rounded px-2 py-1.5 text-xs text-text-primary outline-none"
          >
            <option value="">{t("tm.anyTargetLang")}</option>
            {LANGUAGES.filter((l) => l.code !== "auto").map((lang) => (
              <option key={lang.code} value={lang.code}>
                {lang.name}
              </option>
            ))}
          </select>
        </div>

        {/* Search Results */}
        {searchTotal > 0 && (
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs text-text-secondary">
              <span>{t("tm.resultsFound", { count: searchTotal })}</span>
              <span>{t("tm.pageInfo", { current: searchPage + 1, total: totalPages })}</span>
            </div>

            <div className="max-h-64 overflow-y-auto space-y-1.5">
              {searchResults.map((entry, idx) => (
                <div
                  key={idx}
                  className="bg-bg-secondary rounded-lg p-2.5 text-xs"
                >
                  <div className="flex items-center gap-2 mb-1">
                    <span className="text-text-secondary font-mono">
                      {entry.fromLang} → {entry.toLang}
                    </span>
                    <span className="text-text-secondary/50">•</span>
                    <span className="text-text-secondary">{entry.engine}</span>
                  </div>
                  <p className="text-text-primary mb-1">{entry.source}</p>
                  <p className="text-text-secondary">{entry.target}</p>
                </div>
              ))}
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
              <div className="flex items-center justify-center gap-2">
                <button
                  className="p-1.5 rounded hover:bg-bg-tertiary text-text-secondary disabled:opacity-50"
                  onClick={() => handleSearch(searchPage - 1)}
                  disabled={searchPage === 0 || searching}
                >
                  <ChevronLeft size={14} />
                </button>
                <span className="text-xs text-text-secondary">
                  {searchPage + 1} / {totalPages}
                </span>
                <button
                  className="p-1.5 rounded hover:bg-bg-tertiary text-text-secondary disabled:opacity-50"
                  onClick={() => handleSearch(searchPage + 1)}
                  disabled={searchPage >= totalPages - 1 || searching}
                >
                  <ChevronRight size={14} />
                </button>
              </div>
            )}
          </div>
        )}

        {searchQuery && searchTotal === 0 && !searching && (
          <p className="text-xs text-text-secondary text-center py-2">
            {t("tm.noResults")}
          </p>
        )}
      </div>

      {/* Info */}
      <div className="bg-bg-tertiary rounded-lg p-3 text-xs text-text-secondary">
        <div className="flex items-start gap-2">
          <FileJson size={14} className="mt-0.5 shrink-0" />
          <div>
            <p className="font-medium mb-1">{t("tm.aboutTitle")}</p>
            <p>{t("tm.aboutDesc")}</p>
            <p className="mt-1">{t("tm.supportedFormats") || "支持格式: JSON (原生), TMX (翻译记忆交换标准)"}</p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default TmManager;
