import { useState, useCallback, useEffect } from "react";
import { useTranslateStore } from "../stores/translateStore";
import { ocrScreenRegion, createOverlay } from "../services/ocr";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Scan, X } from "lucide-react";

interface Selection {
  x: number;
  y: number;
  width: number;
  height: number;
}

function OcrSelector() {
  const [isSelecting, setIsSelecting] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [startPos, setStartPos] = useState<{ x: number; y: number } | null>(null);
  const [loading, setLoading] = useState(false);
  const [statusText, setStatusText] = useState("");

  const { translate, setSourceText } = useTranslateStore();

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

    // Save selection and hide window for clean capture
    const sel = { ...selection };
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);
    setLoading(true);
    setStatusText("准备截图...");

    try {
      // Hide the main window so it doesn't appear in capture
      await invoke("hide_main_window");

      // Small delay to ensure window is hidden
      await new Promise((resolve) => setTimeout(resolve, 200));

      setStatusText("OCR识别中...");
      const { text } = await ocrScreenRegion(
        sel.x,
        sel.y,
        sel.width,
        sel.height
      );

      // Show main window again
      await invoke("show_main_window");

      if (text) {
        setSourceText(text);
        setStatusText("翻译中...");
        await translate();

        await createOverlay(
          sel.x,
          sel.y + sel.height + 10,
          Math.max(sel.width, 300),
          150,
          text
        );
      } else {
        setStatusText("未识别到文字");
      }
    } catch (err) {
      console.error("OCR error:", err);
      setStatusText("OCR失败");
      // Make sure to show window again on error
      await invoke("show_main_window").catch(() => {});
    } finally {
      setLoading(false);
      setTimeout(() => setStatusText(""), 2000);
    }
  }, [selection, isSelecting, setSourceText, translate]);

  const startSelection = () => {
    setIsSelecting(true);
    setStatusText("");
  };

  const cancelSelection = () => {
    setIsSelecting(false);
    setStartPos(null);
    setSelection(null);
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

  useEffect(() => {
    const unlisten = listen("trigger-ocr", () => {
      if (!isSelecting && !loading) {
        startSelection();
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [isSelecting, loading]);

  return (
    <>
      <button
        className="bg-primary text-white rounded-lg px-4 py-2 text-sm font-semibold hover:bg-primary-hover transition-colors flex items-center gap-2 disabled:opacity-50"
        onClick={startSelection}
        disabled={isSelecting || loading}
      >
        <Scan size={16} />
        {loading ? statusText : "OCR截图翻译"}
      </button>

      {isSelecting && (
        <div
          className="fixed inset-0 z-50 cursor-crosshair"
          style={{ background: "rgba(0,0,0,0.5)" }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
        >
          <div className="absolute top-4 left-1/2 -translate-x-1/2 bg-bg-secondary border border-border rounded-lg px-4 py-2 text-sm text-text-primary">
            拖拽选择区域，按 ESC 取消
          </div>

          {selection && (
            <div
              className="absolute border-2 border-primary bg-primary/10"
              style={{
                left: selection.x,
                top: selection.y,
                width: selection.width,
                height: selection.height,
              }}
            />
          )}

          <button
            className="absolute top-4 right-4 bg-bg-secondary border border-border text-text-primary rounded-lg p-2 hover:bg-error hover:text-white transition-colors"
            onClick={cancelSelection}
          >
            <X size={20} />
          </button>
        </div>
      )}

      {loading && (
        <div className="fixed bottom-4 right-4 bg-bg-secondary border border-border rounded-lg px-4 py-2 text-sm text-text-primary z-50">
          <div className="animate-pulse">{statusText}</div>
        </div>
      )}
    </>
  );
}

export default OcrSelector;
