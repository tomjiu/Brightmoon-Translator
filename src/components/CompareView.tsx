import { useState, useCallback } from "react";
import { invokeOrThrow } from "../services/invoke";
import { useI18n } from "../i18n";
import { useConfigStore } from "../stores/configStore";
import { Copy, Check, Loader2, Languages } from "lucide-react";
import type { TranslateResponse } from "../types";

interface CompareResult {
  engine: string;
  text: string;
}

function CompareView() {
  const { t } = useI18n();
  const { config } = useConfigStore();
  const [inputText, setInputText] = useState("");
  const [results, setResults] = useState<CompareResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [copiedEngine, setCopiedEngine] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleTranslate = useCallback(async () => {
    if (!inputText.trim()) return;
    setLoading(true);
    setError(null);
    setResults([]);

    try {
      const response = await invokeOrThrow<TranslateResponse>("compare_translate", {
        request: {
          text: inputText.trim(),
          from: config.defaultFrom,
          to: config.defaultTo,
        },
      });

      setResults(response.results);
      if (response.results.length === 0) {
        setError(t("compare.noResults"));
      }
    } catch (err) {
      console.error("Compare translate failed:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [inputText, config.defaultFrom, config.defaultTo, t]);

  const copyText = useCallback((text: string, engine: string) => {
    navigator.clipboard.writeText(text);
    setCopiedEngine(engine);
    setTimeout(() => setCopiedEngine(null), 1500);
  }, []);

  return (
    <div className="flex flex-col h-full gap-3 p-4">
      {/* Header */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4">
        <div className="flex items-center gap-2 mb-3">
          <Languages size={18} className="text-primary" />
          <h3 className="text-sm font-semibold text-text-primary">
            {t("compare.title")}
          </h3>
        </div>
        <p className="text-xs text-text-secondary mb-3">
          {t("compare.description")}
        </p>

        {/* Input */}
        <textarea
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          placeholder={t("compare.placeholder")}
          rows={4}
          className="w-full bg-bg-tertiary text-text-primary border border-border rounded-lg px-3 py-2 text-sm focus:border-primary outline-none resize-none mb-3"
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
              handleTranslate();
            }
          }}
        />

        <button
          className="w-full bg-primary text-white rounded-lg px-4 py-2 text-sm font-semibold hover:bg-primary-hover transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
          onClick={handleTranslate}
          disabled={!inputText.trim() || loading}
        >
          {loading ? (
            <>
              <Loader2 size={14} className="animate-spin" />
              {t("compare.translating")}
            </>
          ) : (
            <>
              <Languages size={14} />
              {t("compare.translate")}
            </>
          )}
        </button>
      </div>

      {/* Error */}
      {error && (
        <div className="bg-error/10 border border-error/30 rounded-lg p-3 text-sm text-error">
          {error}
        </div>
      )}

      {/* Results */}
      {results.length > 0 && (
        <div className="flex-1 overflow-y-auto space-y-2 min-h-0">
          {results.map((result) => (
            <div
              key={result.engine}
              className="bg-bg-secondary border border-border rounded-lg p-3 group hover:border-primary/30 transition-colors"
            >
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-medium text-primary bg-primary/10 px-2 py-0.5 rounded-full">
                  {result.engine}
                </span>
                <button
                  className="p-1 rounded hover:bg-bg-tertiary text-text-secondary opacity-0 group-hover:opacity-100 transition-opacity"
                  onClick={() => copyText(result.text, result.engine)}
                  title={t("compare.copy")}
                >
                  {copiedEngine === result.engine ? (
                    <Check size={14} className="text-success" />
                  ) : (
                    <Copy size={14} />
                  )}
                </button>
              </div>
              <p className="text-sm text-text-primary leading-relaxed select-text">
                {result.text}
              </p>
            </div>
          ))}
        </div>
      )}

      {/* Empty state */}
      {results.length === 0 && !loading && !error && (
        <div className="flex-1 flex flex-col items-center justify-center text-text-secondary">
          <Languages size={48} className="mb-4 opacity-30" />
          <p className="text-sm">{t("compare.empty")}</p>
          <p className="text-xs mt-1">{t("compare.emptyHint")}</p>
        </div>
      )}
    </div>
  );
}

export default CompareView;
