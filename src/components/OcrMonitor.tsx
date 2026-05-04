import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useOcrMonitor } from "../hooks/useOcrMonitor";
import { useI18n } from "../i18n";
import {
  Scan,
  X,
  Square,
  Pin,
  MousePointerClick,
  Clock,
} from "lucide-react";

interface Selection {
  x: number;
  y: number;
  width: number;
  height: number;
}

function OcrMonitor() {
  const [isSelecting, setIsSelecting] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [startPos, setStartPos] = useState<{ x: number; y: number } | null>(
    null
  );
  const [interval, setInterval_] = useState(2000);

  const { t } = useI18n();

  const {
    isMonitoring,
    region,
    lastText,
    clickThrough,
    pinned,
    startMonitoring,
    stopMonitoring,
    toggleClickThrough,
    togglePin,
  } = useOcrMonitor();

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!isSelecting) return;
      setStartPos({ x: e.clientX, y: e.clientY });
      setSelection(null);
    },
    [isSelecting]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!startPos || !isSelecting) return;
      const x = Math.min(startPos.x, e.clientX);
      const y = Math.min(startPos.y, e.clientY);
      const width = Math.abs(e.clientX - startPos.x);
      const height = Math.abs(e.clientY - startPos.y);
      setSelection({ x, y, width, height });
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

    const sel = { ...selection };
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);

    // Hide window for clean capture
    await invoke("hide_main_window");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Start monitoring
    startMonitoring(sel, interval);

    // Show window again
    await invoke("show_main_window");
  }, [selection, isSelecting, interval, startMonitoring]);

  const cancelSelection = () => {
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);
  };

  const handleStartSelection = () => {
    setIsSelecting(true);
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
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-success animate-pulse" />
              <span className="text-xs text-success">{t("ocr.monitoring")}</span>
            </div>
          )}
        </div>

        {isMonitoring && region ? (
          /* Monitoring Active View */
          <div className="space-y-3">
            {/* Region Info */}
            <div className="bg-bg-tertiary rounded-lg p-3">
              <div className="text-xs text-text-secondary mb-2">{t("ocr.region")}</div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div>
                  <span className="text-text-secondary">{t("ocr.x")}: </span>
                  <span className="text-text-primary">{region.x}</span>
                </div>
                <div>
                  <span className="text-text-secondary">{t("ocr.y")}: </span>
                  <span className="text-text-primary">{region.y}</span>
                </div>
                <div>
                  <span className="text-text-secondary">{t("ocr.width")}: </span>
                  <span className="text-text-primary">{region.width}</span>
                </div>
                <div>
                  <span className="text-text-secondary">{t("ocr.height")}: </span>
                  <span className="text-text-primary">{region.height}</span>
                </div>
              </div>
            </div>

            {/* Last OCR Text */}
            {lastText && (
              <div className="bg-bg-tertiary rounded-lg p-3">
                <div className="text-xs text-text-secondary mb-2">
                  {t("ocr.lastText")}
                </div>
                <div className="text-sm text-text-primary line-clamp-3">
                  {lastText}
                </div>
              </div>
            )}

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
                {clickThrough ? t("ocr.clickThroughOn") : t("ocr.clickThrough")}
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
                  <span className="text-xs text-text-secondary">{t("ocr.interval")}</span>
                </div>
                <span className="text-xs text-primary font-medium">
                  {interval / 1000}{t("ocr.seconds")}
                </span>
              </div>
              <input
                type="range"
                min="500"
                max="10000"
                step="500"
                value={interval}
                onChange={(e) => setInterval_(Number(e.target.value))}
                className="w-full accent-primary"
              />
              <div className="flex justify-between text-xs text-text-secondary mt-1">
                <span>{t("ocr.intervalMin")}</span>
                <span>{t("ocr.intervalMax")}</span>
              </div>
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
                  left: selection.x,
                  top: selection.y,
                  width: selection.width,
                  height: selection.height,
                }}
              />
              <div
                className="absolute bg-bg-secondary border border-border rounded px-2 py-1 text-xs text-text-primary"
                style={{
                  left: selection.x,
                  top: selection.y - 28,
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
