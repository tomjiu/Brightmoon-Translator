import { useState, useEffect, useRef, useCallback } from "react";
import { invokeOrThrow } from "../services/invoke";
import { listen } from "@tauri-apps/api/event";
import { useConfigStore } from "../stores/configStore";
import { useI18n } from "../i18n";
import { LANGUAGES } from "../types";
import type {
  BatchConfig,
  BatchTask,
  BatchProgress,
  BatchJobStatus,
} from "../types";
import {
  Play,
  Pause,
  Square,
  Download,
  Copy,
  Check,
  FileText,
  Loader2,
  AlertCircle,
  CheckCircle2,
  XCircle,
  RotateCcw,
  RefreshCw,
} from "lucide-react";

function BatchTranslator() {
  const config = useConfigStore((s) => s.config);
  const { t } = useI18n();
  const [inputText, setInputText] = useState("");
  const [fromLang, setFromLang] = useState(config.defaultFrom || "auto");
  const [toLang, setToLang] = useState(config.defaultTo || "zh");
  const [concurrency, setConcurrency] = useState(3);
  const [continueOnError, setContinueOnError] = useState(true);
  const [jobStatus, setJobStatus] = useState<BatchJobStatus>("idle");
  const [progress, setProgress] = useState<BatchProgress | null>(null);
  const [results, setResults] = useState<BatchTask[]>([]);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Listen for batch progress events
  useEffect(() => {
    let unlisten1: (() => void) | null = null;
    let unlisten2: (() => void) | null = null;

    const setup = async () => {
      unlisten1 = await listen<BatchProgress>("batch-progress", (event) => {
        setProgress(event.payload);
        setJobStatus(event.payload.status);
      });

      unlisten2 = await listen<BatchTask>("batch-task-complete", (event) => {
        setResults((prev) => {
          const idx = prev.findIndex((r) => r.id === event.payload.id);
          if (idx >= 0) {
            const next = [...prev];
            next[idx] = event.payload;
            return next;
          }
          return [...prev, event.payload];
        });
      });
    };

    setup();
    return () => {
      unlisten1?.();
      unlisten2?.();
    };
  }, []);

  // Parse input text into lines
  const textSegments = inputText
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);

  const handleSubmit = useCallback(async () => {
    if (textSegments.length === 0) return;

    setResults([]);
    setProgress(null);
    setJobStatus("running");

    const batchConfig: BatchConfig = {
      concurrency,
      fromLang,
      toLang,
      continueOnError,
    };

    try {
      await invokeOrThrow<string>("batch_submit", {
        texts: textSegments,
        config: batchConfig,
      });
    } catch (err) {
      console.error("Batch submit failed:", err);
      setJobStatus("failed");
    }
  }, [textSegments, fromLang, toLang, concurrency, continueOnError]);

  const handleCancel = useCallback(async () => {
    try {
      await invokeOrThrow("batch_cancel");
      setJobStatus("cancelled");
    } catch (err) {
      console.error("Batch cancel failed:", err);
    }
  }, []);

  const handlePause = useCallback(async () => {
    try {
      await invokeOrThrow("batch_pause");
      setJobStatus("paused");
    } catch (err) {
      console.error("Batch pause failed:", err);
    }
  }, []);

  const handleResume = useCallback(async () => {
    try {
      await invokeOrThrow("batch_resume");
      setJobStatus("running");
    } catch (err) {
      console.error("Batch resume failed:", err);
    }
  }, []);

  const handleRetryFailed = useCallback(async () => {
    try {
      await invokeOrThrow("batch_retry_failed");
      setJobStatus("running");
    } catch (err) {
      console.error("Batch retry failed:", err);
    }
  }, []);

  const handleReset = useCallback(async () => {
    try {
      await invokeOrThrow("batch_reset");
      setJobStatus("idle");
      setProgress(null);
      setResults([]);
      setInputText("");
    } catch (err) {
      console.error("Batch reset failed:", err);
    }
  }, []);

  const handleExport = useCallback(() => {
    const lines = results
      .filter((r) => r.status === "completed" && r.result)
      .map((r) => `${r.text}\t${r.result}`);
    const blob = new Blob([lines.join("\n")], {
      type: "text/plain;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `batch-translations-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }, [results]);

  const copyText = useCallback((text: string, id: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1500);
  }, []);

  const copyAllResults = useCallback(() => {
    const allText = results
      .filter((r) => r.result)
      .map((r) => r.result)
      .join("\n");
    navigator.clipboard.writeText(allText);
    setCopiedId("all");
    setTimeout(() => setCopiedId(null), 1500);
  }, [results]);

  const isRunning = jobStatus === "running";
  const isPaused = jobStatus === "paused";
  const isCompleted = jobStatus === "completed" || jobStatus === "failed";
  const completedCount = results.filter((r) => r.status === "completed").length;
  const failedCount = results.filter((r) => r.status === "failed").length;

  return (
    <div className="flex flex-col h-full gap-3 p-4">
      {/* Header */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <FileText size={18} className="text-primary" />
            <h3 className="text-sm font-semibold text-text-primary">
              {t("batch.title")}
            </h3>
          </div>
          {progress && (
            <span className="text-xs text-text-secondary bg-bg-tertiary px-2 py-0.5 rounded-full">
              {t("batch.progress", { completed: progress.completed, total: progress.total })}
              {progress.failed > 0 && (
                <span className="text-error ml-1">{t("batch.failedCount", { count: progress.failed })}</span>
              )}
            </span>
          )}
        </div>

        <p className="text-xs text-text-secondary mb-3">
          {t("batch.description")}
        </p>

        {/* Language Selection */}
        <div className="flex gap-2 mb-3">
          <select
            value={fromLang}
            onChange={(e) => setFromLang(e.target.value)}
            className="flex-1 bg-bg-tertiary border border-border rounded px-2 py-1.5 text-xs text-text-primary outline-none"
            disabled={isRunning}
          >
            {LANGUAGES.map((lang) => (
              <option key={lang.code} value={lang.code}>
                {lang.name}
              </option>
            ))}
          </select>
          <span className="text-text-secondary self-center">→</span>
          <select
            value={toLang}
            onChange={(e) => setToLang(e.target.value)}
            className="flex-1 bg-bg-tertiary border border-border rounded px-2 py-1.5 text-xs text-text-primary outline-none"
            disabled={isRunning}
          >
            {LANGUAGES.filter((l) => l.code !== "auto").map((lang) => (
              <option key={lang.code} value={lang.code}>
                {lang.name}
              </option>
            ))}
          </select>
        </div>

        {/* Concurrency Setting */}
        <div className="flex items-center gap-3 mb-3">
          <label className="flex items-center gap-2 text-xs text-text-secondary">
            {t("batch.concurrency")}
            <input
              type="number"
              min={1}
              max={10}
              value={concurrency}
              onChange={(e) => setConcurrency(Number(e.target.value))}
              className="w-16 bg-bg-tertiary border border-border rounded px-2 py-1 text-xs text-text-primary outline-none"
              disabled={isRunning}
            />
          </label>
          <label className="flex items-center gap-1.5 cursor-pointer">
            <input
              type="checkbox"
              checked={continueOnError}
              onChange={(e) => setContinueOnError(e.target.checked)}
              className="accent-primary w-3.5 h-3.5"
              disabled={isRunning}
            />
            <span className="text-xs text-text-secondary">{t("batch.continueOnError")}</span>
          </label>
        </div>

        {/* Input Text Area */}
        <textarea
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          placeholder={t("translator.placeholder") + "\n\nExample:\nThis is a test text.\nThis is the second text."}
          className="w-full h-32 bg-bg-tertiary border border-border rounded-lg p-3 text-sm text-text-primary outline-none focus:border-primary resize-none placeholder:text-text-secondary/50"
          disabled={isRunning}
        />

        {/* Segment Count */}
        {textSegments.length > 0 && (
          <div className="text-xs text-text-secondary mt-1">
            {t("batch.segmentCount", { count: textSegments.length })}
          </div>
        )}

        {/* Progress Bar */}
        {progress && progress.total > 0 && (
          <div className="mt-3">
            <div className="w-full h-2 bg-bg-tertiary rounded-full overflow-hidden">
              <div
                className="h-full bg-primary transition-all duration-300"
                style={{
                  width: `${((progress.completed + progress.failed) / progress.total) * 100}%`,
                }}
              />
            </div>
            <div className="flex justify-between text-[10px] text-text-secondary mt-1">
              <span>
                {t("batch.completed")}: {progress.completed} | {t("batch.failed")}: {progress.failed}
              </span>
              <span>
                {Math.round(
                  ((progress.completed + progress.failed) / progress.total) * 100
                )}
                %
              </span>
            </div>
          </div>
        )}

        {/* Action Buttons */}
        <div className="flex gap-2 mt-3">
          {/* Main Action Button */}
          {isRunning ? (
            <>
              <button
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-warning text-white hover:bg-warning/90 transition-colors"
                onClick={handlePause}
              >
                <Pause size={14} />
                {t("batch.pause")}
              </button>
              <button
                className="flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-error text-white hover:bg-error/90 transition-colors"
                onClick={handleCancel}
              >
                <Square size={14} />
              </button>
            </>
          ) : isPaused ? (
            <>
              <button
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-primary text-white hover:bg-primary-hover transition-colors"
                onClick={handleResume}
              >
                <Play size={14} />
                {t("batch.resume")}
              </button>
              <button
                className="flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-error text-white hover:bg-error/90 transition-colors"
                onClick={handleCancel}
              >
                <Square size={14} />
              </button>
            </>
          ) : (
            <button
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg text-sm font-semibold bg-primary text-white hover:bg-primary-hover transition-colors disabled:opacity-50"
              onClick={handleSubmit}
              disabled={textSegments.length === 0}
            >
              <Play size={14} />
              {t("batch.start")}
            </button>
          )}

          {/* Retry Failed Button */}
          {isCompleted && failedCount > 0 && (
            <button
              className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-warning/10 text-warning hover:bg-warning/20 transition-colors"
              onClick={handleRetryFailed}
            >
              <RefreshCw size={13} />
              {t("batch.retry")}
            </button>
          )}

          {/* Copy Button */}
          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={copyAllResults}
            disabled={completedCount === 0}
          >
            {copiedId === "all" ? (
              <Check size={13} className="text-success" />
            ) : (
              <Copy size={13} />
            )}
          </button>

          {/* Export Button */}
          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleExport}
            disabled={completedCount === 0}
          >
            <Download size={13} />
          </button>

          {/* Reset Button */}
          <button
            className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium bg-bg-tertiary text-text-secondary hover:text-text-primary hover:bg-bg-tertiary/80 transition-colors disabled:opacity-50"
            onClick={handleReset}
            disabled={isRunning || isPaused}
          >
            <RotateCcw size={13} />
          </button>
        </div>
      </div>

      {/* Results List */}
      <div
        ref={listRef}
        className="flex-1 overflow-y-auto space-y-2 min-h-0"
      >
        {results.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-secondary">
            <FileText size={48} className="mb-4 opacity-30" />
            <p className="text-sm">{t("batch.noResults")}</p>
            <p className="text-xs mt-1">{t("batch.noResultsHint")}</p>
          </div>
        ) : (
          results.map((task) => (
            <div
              key={task.id}
              className="bg-bg-secondary border border-border rounded-lg p-3 group hover:border-primary/30 transition-colors"
            >
              {/* Header */}
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-1.5">
                  <span className="text-[10px] text-text-secondary bg-bg-tertiary px-1.5 py-0.5 rounded">
                    #{task.index + 1}
                  </span>
                  {task.status === "completed" && (
                    <CheckCircle2 size={12} className="text-success" />
                  )}
                  {task.status === "failed" && (
                    <XCircle size={12} className="text-error" />
                  )}
                  {task.status === "running" && (
                    <Loader2 size={12} className="text-primary animate-spin" />
                  )}
                </div>
                {task.result && (
                  <button
                    className="p-1 rounded hover:bg-bg-tertiary text-text-secondary opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={() => copyText(task.result!, task.id)}
                  >
                    {copiedId === task.id ? (
                      <Check size={12} className="text-success" />
                    ) : (
                      <Copy size={12} />
                    )}
                  </button>
                )}
              </div>

              {/* Source text */}
              <div className="text-xs text-text-secondary mb-1.5 line-clamp-2">
                {task.text}
              </div>

              {/* Result or error */}
              {task.result ? (
                <div className="text-sm text-text-primary leading-relaxed">
                  {task.result}
                </div>
              ) : task.error ? (
                <div className="text-xs text-error flex items-center gap-1">
                  <AlertCircle size={12} />
                  {task.error}
                </div>
              ) : null}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export default BatchTranslator;
