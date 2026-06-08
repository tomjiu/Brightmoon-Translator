import { useState, useCallback, useEffect } from "react";
import { invokeOrThrow } from "../services/invoke";
import { useOcrMonitor } from "../hooks/useOcrMonitor";
import { useI18n } from "../i18n";
import { useConfigStore } from "../stores/configStore";
import {
  Scan,
  X,
  Square,
  Pin,
  MousePointerClick,
  Clock,
  Pause,
  Play,
  RefreshCw,
  Link,
  Unlink,
  Activity,
} from "lucide-react";

interface Selection {
  x: number;
  y: number;
  width: number;
  height: number;
  /** CSS left for overlay display (clientX-based) */
  cssX: number;
  /** CSS top for overlay display (clientY-based) */
  cssY: number;
}

function OcrMonitor() {
  const [isSelecting, setIsSelecting] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [startPos, setStartPos] = useState<{ x: number; y: number; clientX: number; clientY: number } | null>(
    null
  );
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const [interval, setInterval_] = useState(config.ocrInterval ?? 2000);

  // Sync interval from config when config loads
  useEffect(() => {
    if (config.ocrInterval !== undefined) {
      setInterval_(config.ocrInterval);
    }
  }, [config.ocrInterval]);

  const { t } = useI18n();

  const {
    isMonitoring,
    paused,
    autoPaused,
    region,
    lastText,
    lastGoodText,
    clickThrough,
    pinned,
    boundWindow,
    cycleCount,
    skipCount,
    lastDiag,
    startMonitoring,
    stopMonitoring,
    pauseMonitoring,
    resumeMonitoring,
    toggleClickThrough,
    togglePin,
    rebindWindow,
    unbindWindow,
  } = useOcrMonitor();

  const [showDiag, setShowDiag] = useState(false);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!isSelecting) return;
      setStartPos({
        x: e.screenX,
        y: e.screenY,
        clientX: e.clientX,
        clientY: e.clientY,
      });
      setSelection(null);
    },
    [isSelecting]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!startPos || !isSelecting) return;
      // Screen coordinates for Rust capture
      const x = Math.min(startPos.x, e.screenX);
      const y = Math.min(startPos.y, e.screenY);
      const width = Math.abs(e.screenX - startPos.x);
      const height = Math.abs(e.screenY - startPos.y);
      // Client coordinates for CSS overlay display
      const cssX = Math.min(startPos.clientX, e.clientX);
      const cssY = Math.min(startPos.clientY, e.clientY);
      setSelection({ x, y, width, height, cssX, cssY });
    },
    [startPos, isSelecting]
  );

  const handleMouseUp = useCallback(async () => {
    if (!selection || !isSelecting) return;
    if (selection.width < 20 || selection.height < 20) {
      setSelection(null);
      setStartPos(null);
      return;
    }

    // Extract screen coordinates for Rust capture
    const region = {
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    };
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);

    // Hide window for clean capture
    await invokeOrThrow("hide_main_window");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Start monitoring with screen coordinates
    startMonitoring(region, interval);

    // Show window again
    await invokeOrThrow("show_main_window");
  }, [selection, isSelecting, interval, startMonitoring]);

  const cancelSelection = () => {
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);
  };

  const handleStartSelection = () => {
    setIsSelecting(true);
  };

  const handleReselect = () => {
    stopMonitoring();
    setIsSelecting(true);
  };

  const handleRebind = async () => {
    if (region) {
      await invokeOrThrow("show_main_window");
      await new Promise((resolve) => setTimeout(resolve, 300));
      await invokeOrThrow("hide_main_window");
      await new Promise((resolve) => setTimeout(resolve, 200));
      await rebindWindow(region);
      await invokeOrThrow("show_main_window");
    }
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        cancelSelection();
      }
    };

    if (isSelecting) {
      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
    }
  }, [isSelecting]);

  return (
    <>
      {/* Main Control Panel */}
      <div className="bg-bg-secondary border border-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <Scan size={18} className="text-primary" />
            <h3 className="text-sm font-semibold text-text-primary">
              {t("ocr.title")}
            </h3>
          </div>
          {isMonitoring && (
            <div className="flex items-center gap-2">
              <div className="flex items-center gap-1">
                <Activity size={12} className="text-text-secondary" />
                <span className="text-xs text-text-secondary">
                  {cycleCount}
                </span>
              </div>
              {skipCount > 0 && (
                <div className="flex items-center gap-1">
                  <span className="text-xs text-warning">
                    skip:{skipCount}
                  </span>
                </div>
              )}
              <div
                className={`w-2 h-2 rounded-full ${
                  autoPaused
                    ? "bg-text-secondary"
                    : paused
                      ? "bg-warning"
                      : "bg-success animate-pulse"
                }`}
              />
              <span
                className={`text-xs ${
                  autoPaused
                    ? "text-text-secondary"
                    : paused
                      ? "text-warning"
                      : "text-success"
                }`}
              >
                {autoPaused
                  ? t("ocr.autoPaused")
                  : paused
                    ? t("ocr.paused")
                    : t("ocr.monitoring")}
              </span>
            </div>
          )}
        </div>

        {isMonitoring && region ? (
          /* Monitoring Active View */
          <div className="space-y-3">
            {/* Bound Window Info */}
            {boundWindow && (
              <div className="bg-bg-tertiary rounded-lg p-3">
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-1.5">
                    <Link size={12} className="text-primary" />
                    <span className="text-xs text-text-secondary">
                      {t("ocr.boundTo")}
                    </span>
                  </div>
                  <div className="flex items-center gap-1">
                    <button
                      className="text-xs text-primary hover:text-primary-hover px-1.5 py-0.5 rounded bg-primary/10 hover:bg-primary/20 transition-colors"
                      onClick={handleRebind}
                      title={t("ocr.rebind")}
                    >
                      <RefreshCw size={10} />
                    </button>
                    <button
                      className="text-xs text-error hover:text-error/80 px-1.5 py-0.5 rounded bg-error/10 hover:bg-error/20 transition-colors"
                      onClick={unbindWindow}
                      title={t("ocr.unbind")}
                    >
                      <Unlink size={10} />
                    </button>
                  </div>
                </div>
                <div className="text-xs text-text-primary truncate">
                  {boundWindow.title || `HWND ${boundWindow.hwnd}`}
                </div>
              </div>
            )}

            {/* Region Info */}
            <div className="bg-bg-tertiary rounded-lg p-3">
              <div className="text-xs text-text-secondary mb-2">
                {t("ocr.region")}
              </div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div>
                  <span className="text-text-secondary">{t("ocr.x")}: </span>
                  <span className="text-text-primary">
                    {Math.round(region.x)}
                  </span>
                </div>
                <div>
                  <span className="text-text-secondary">{t("ocr.y")}: </span>
                  <span className="text-text-primary">
                    {Math.round(region.y)}
                  </span>
                </div>
                <div>
                  <span className="text-text-secondary">
                    {t("ocr.width")}:{" "}
                  </span>
                  <span className="text-text-primary">{region.width}</span>
                </div>
                <div>
                  <span className="text-text-secondary">
                    {t("ocr.height")}:{" "}
                  </span>
                  <span className="text-text-primary">{region.height}</span>
                </div>
              </div>
            </div>

            {/* Last OCR Text */}
            {(lastText || lastGoodText) && (
              <div className="bg-bg-tertiary rounded-lg p-3">
                <div className="text-xs text-text-secondary mb-2">
                  {t("ocr.lastText")}
                </div>
                <div className="text-sm text-text-primary line-clamp-3">
                  {lastText || lastGoodText}
                </div>
                {lastText && lastGoodText && lastText !== lastGoodText && (
                  <div className="text-xs text-text-secondary mt-1 italic">
                    {t("ocr.lastGoodText")}: {lastGoodText.slice(0, 50)}
                  </div>
                )}
              </div>
            )}

            {/* Diagnostics Panel */}
            <div className="bg-bg-tertiary rounded-lg overflow-hidden">
              <button
                className="w-full flex items-center justify-between px-3 py-2 text-xs text-text-secondary hover:text-text-primary transition-colors"
                onClick={() => setShowDiag(!showDiag)}
              >
                <span className="flex items-center gap-1.5">
                  <Activity size={12} />
                  {t("ocr.diagnostics")}
                </span>
                <span className="text-[10px]">{showDiag ? "▲" : "▼"}</span>
              </button>
              {showDiag && lastDiag && (
                <div className="px-3 pb-3 grid grid-cols-2 gap-2 text-xs">
                  <div>
                    <span className="text-text-secondary">{t("ocr.captureMs")}: </span>
                    <span className="text-text-primary">{lastDiag.captureMs.toFixed(0)}ms</span>
                  </div>
                  <div>
                    <span className="text-text-secondary">{t("ocr.ocrMs")}: </span>
                    <span className="text-text-primary">{lastDiag.ocrMs.toFixed(0)}ms</span>
                  </div>
                  <div>
                    <span className="text-text-secondary">{t("ocr.translateMs")}: </span>
                    <span className="text-text-primary">{lastDiag.translateMs.toFixed(0)}ms</span>
                  </div>
                  <div>
                    <span className="text-text-secondary">{t("ocr.qualityScore")}: </span>
                    <span className="text-text-primary">{lastDiag.qualityScore.toFixed(2)}</span>
                  </div>
                  <div>
                    <span className="text-text-secondary">{t("ocr.textLen")}: </span>
                    <span className="text-text-primary">{lastDiag.textLen}</span>
                  </div>
                  <div>
                    <span className="text-text-secondary">{t("ocr.skipReason")}: </span>
                    <span className={lastDiag.skipped ? "text-warning" : "text-success"}>
                      {lastDiag.skipped ? lastDiag.skipReason : "none"}
                    </span>
                  </div>
                </div>
              )}
              {showDiag && !lastDiag && (
                <div className="px-3 pb-3 text-xs text-text-secondary">
                  {t("ocr.noDiagData")}
                </div>
              )}
            </div>

            {/* Control Buttons */}
            <div className="flex flex-wrap gap-2">
              <button
                className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  clickThrough
                    ? "bg-primary text-white"
                    : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
                }`}
                onClick={toggleClickThrough}
              >
                <MousePointerClick size={14} />
                {clickThrough
                  ? t("ocr.clickThroughOn")
                  : t("ocr.clickThrough")}
              </button>

              <button
                className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                  pinned
                    ? "bg-warning text-white"
                    : "bg-bg-tertiary text-text-secondary hover:bg-bg-tertiary/80"
                }`}
                onClick={togglePin}
              >
                <Pin size={14} />
                {pinned ? t("ocr.pinned") : t("ocr.pin")}
              </button>
            </div>

            {/* Pause/Resume/Stop/Reselect Buttons */}
            <div className="flex flex-wrap gap-2">
              {!paused ? (
                <button
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium bg-warning/20 text-warning hover:bg-warning/30 transition-colors"
                  onClick={pauseMonitoring}
                >
                  <Pause size={14} />
                  {t("ocr.pause")}
                </button>
              ) : (
                <button
                  className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium bg-success/20 text-success hover:bg-success/30 transition-colors"
                  onClick={resumeMonitoring}
                >
                  <Play size={14} />
                  {t("ocr.resume")}
                </button>
              )}

              <button
                className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium bg-accent/20 text-accent hover:bg-accent/30 transition-colors"
                onClick={handleReselect}
              >
                <RefreshCw size={14} />
                {t("ocr.reselect")}
              </button>

              <button
                className="flex-1 flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium bg-error/20 text-error hover:bg-error/30 transition-colors"
                onClick={stopMonitoring}
              >
                <Square size={14} />
                {t("ocr.stop")}
              </button>
            </div>
          </div>
        ) : (
          /* Setup View */
          <div className="space-y-3">
            {/* Interval Setting */}
            <div className="bg-bg-tertiary rounded-lg p-3">
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <Clock size={14} className="text-text-secondary" />
                  <span className="text-xs text-text-secondary">
                    {t("ocr.interval")}
                  </span>
                </div>
                <span className="text-xs text-primary font-medium">
                  {interval / 1000}
                  {t("ocr.seconds")}
                </span>
              </div>
              <input
                type="range"
                min="500"
                max="10000"
                step="500"
                value={interval}
                onChange={(e) => {
                  const val = Number(e.target.value);
                  setInterval_(val);
                  updateConfig((prev) => ({ ...prev, ocrInterval: val }));
                }}
                className="w-full accent-primary"
              />
              <div className="flex justify-between text-xs text-text-secondary mt-1">
                <span>{t("ocr.intervalMin")}</span>
                <span>{t("ocr.intervalMax")}</span>
              </div>
            </div>

            {/* OCR Settings */}
            <div className="bg-bg-tertiary rounded-lg p-3 space-y-2.5">
              <label className="flex items-start gap-2.5 cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.ocrAutoBindWindow ?? true}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      ocrAutoBindWindow: e.target.checked,
                    }))
                  }
                  className="mt-0.5 accent-primary"
                />
                <div>
                  <div className="text-xs text-text-primary">
                    {t("ocr.autoBindWindow")}
                  </div>
                  <div className="text-[11px] text-text-secondary mt-0.5">
                    {t("ocr.autoBindWindowHint")}
                  </div>
                </div>
              </label>
              <label className="flex items-start gap-2.5 cursor-pointer">
                <input
                  type="checkbox"
                  checked={config.ocrClickThrough ?? false}
                  onChange={(e) =>
                    updateConfig((prev) => ({
                      ...prev,
                      ocrClickThrough: e.target.checked,
                    }))
                  }
                  className="mt-0.5 accent-primary"
                />
                <div>
                  <div className="text-xs text-text-primary">
                    {t("ocr.clickThroughSetting")}
                  </div>
                  <div className="text-[11px] text-text-secondary mt-0.5">
                    {t("ocr.clickThroughSettingHint")}
                  </div>
                </div>
              </label>
            </div>

            {/* Info Text */}
            <div className="text-xs text-text-secondary bg-bg-tertiary/50 rounded-lg p-3">
              <p className="mb-1">{t("ocr.description")}</p>
              <ul className="list-disc list-inside space-y-0.5 ml-1">
                <li>{t("ocr.feature1")}</li>
                <li>{t("ocr.feature2")}</li>
                <li>{t("ocr.feature3")}</li>
              </ul>
            </div>

            {/* Start Button */}
            <button
              className="w-full bg-primary text-white rounded-lg px-4 py-2.5 text-sm font-semibold hover:bg-primary-hover transition-colors flex items-center justify-center gap-2"
              onClick={handleStartSelection}
            >
              <Scan size={16} />
              {t("ocr.start")}
            </button>
          </div>
        )}
      </div>

      {/* Selection Overlay */}
      {isSelecting && (
        <div
          className="fixed inset-0 z-50 cursor-crosshair"
          style={{ background: "rgba(0,0,0,0.5)" }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
        >
          <div className="absolute top-4 left-1/2 -translate-x-1/2 bg-bg-secondary border border-border rounded-lg px-4 py-2 text-sm text-text-primary">
            {t("ocr.selectHint")}
          </div>

          {selection && (
            <>
              <div
                className="absolute border-2 border-accent bg-accent/10"
                style={{
                  left: selection.cssX,
                  top: selection.cssY,
                  width: selection.width,
                  height: selection.height,
                }}
              />
              <div
                className="absolute bg-bg-secondary border border-border rounded px-2 py-1 text-xs text-text-primary"
                style={{
                  left: selection.cssX,
                  top: selection.cssY - 28,
                }}
              >
                {Math.round(selection.width)} x {Math.round(selection.height)}
              </div>
            </>
          )}

          <button
            className="absolute top-4 right-4 bg-bg-secondary border border-border text-text-primary rounded-lg p-2 hover:bg-error hover:text-white transition-colors"
            onClick={cancelSelection}
          >
            <X size={20} />
          </button>
        </div>
      )}
    </>
  );
}

export default OcrMonitor;
