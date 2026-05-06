import { useEffect, useRef, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { RefreshCw, Play, Pause, X } from "lucide-react";

export default function OcrRegionFrame() {
  const win = getCurrentWindow();
  const [continuous, setContinuous] = useState(false);
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef({ x: 0, y: 0, winX: 0, winY: 0 });

  useEffect(() => {
    // Emit initial position once window is shown
    const timer = setTimeout(async () => {
      try {
        const pos = await win.outerPosition();
        const size = await win.outerSize();
        await emit("ocr-region-position-changed", {
          x: pos.x,
          y: pos.y,
          width: size.width,
          height: size.height,
        });
      } catch {
        // window may already be closed
      }
    }, 200);
    return () => clearTimeout(timer);
  }, []);

  // ---- Drag anywhere on the frame ----
  const onMouseDown = async (e: React.MouseEvent) => {
    // Don't drag if clicking on a button
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    setDragging(true);
    try {
      const pos = await win.outerPosition();
      const size = await win.outerSize();
      dragStart.current = {
        x: e.screenX,
        y: e.screenY,
        winX: pos.x,
        winY: pos.y,
      };
      // Store size for position-changed event
      (dragStart.current as Record<string, unknown>).w = size.width;
      (dragStart.current as Record<string, unknown>).h = size.height;
    } catch {
      setDragging(false);
    }
  };

  useEffect(() => {
    if (!dragging) return;

    const onMouseMove = (e: MouseEvent) => {
      const dx = e.screenX - dragStart.current.x;
      const dy = e.screenY - dragStart.current.y;
      const newX = dragStart.current.winX + dx;
      const newY = dragStart.current.winY + dy;
      void win.setPosition({ type: "Physical", x: newX, y: newY } as never);
    };

    const onMouseUp = async () => {
      setDragging(false);
      try {
        const pos = await win.outerPosition();
        const size = await win.outerSize();
        await emit("ocr-region-position-changed", {
          x: pos.x,
          y: pos.y,
          width: size.width,
          height: size.height,
        });
      } catch {
        // window may already be closed
      }
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [dragging]);

  // ---- Resize from corner handle ----
  const resizing = useRef(false);

  const onResizeStart = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    resizing.current = true;

    const startX = e.screenX;
    const startY = e.screenY;

    const doResize = (curX: number, curY: number) => {
      const dx = curX - startX;
      const dy = curY - startY;
      void win.outerSize().then((size) => {
        const newW = Math.max(80, size.width + dx);
        const newH = Math.max(60, size.height + dy);
        void win.setSize({ type: "Logical", width: newW, height: newH } as never);
        // Update startX/startY for next delta
        dragStart.current.x = curX;
        dragStart.current.y = curY;
      });
    };

    const onMove = (ev: MouseEvent) => {
      if (!resizing.current) return;
      doResize(ev.screenX, ev.screenY);
    };

    const onUp = async () => {
      resizing.current = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      try {
        const pos = await win.outerPosition();
        const size = await win.outerSize();
        await emit("ocr-region-size-changed", {
          x: pos.x,
          y: pos.y,
          width: size.width,
          height: size.height,
        });
      } catch {
        // window may already be closed
      }
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  // ---- Button handlers ----
  const handleRefresh = () => {
    void emit("ocr-region-refresh", null);
  };

  const handleToggleContinuous = () => {
    const next = !continuous;
    setContinuous(next);
    void emit("ocr-region-continuous", { enabled: next });
  };

  const handleClose = () => {
    void emit("ocr-region-close", null);
    void win.close();
  };

  return (
    <div
      className="fixed inset-0 select-none"
      style={{ cursor: dragging ? "grabbing" : "default" }}
      onMouseDown={onMouseDown}
    >
      {/* Visible border frame */}
      <div className="absolute inset-0 border-2 border-sky-400 pointer-events-none" />

      {/* Top control bar */}
      <div className="absolute top-0 left-0 right-0 flex items-center justify-center gap-1 pt-1 pointer-events-none">
        <div className="flex items-center gap-1 rounded-b-lg bg-gray-900/80 px-2 py-1 pointer-events-auto">
          <button
            className="flex items-center justify-center w-6 h-6 rounded text-white/80 hover:text-white hover:bg-white/20 transition-colors"
            onClick={handleRefresh}
            title="刷新 OCR"
          >
            <RefreshCw size={14} />
          </button>
          <button
            className={`flex items-center justify-center w-6 h-6 rounded transition-colors ${
              continuous
                ? "text-sky-400 bg-sky-400/20"
                : "text-white/80 hover:text-white hover:bg-white/20"
            }`}
            onClick={handleToggleContinuous}
            title={continuous ? "暂停持续刷新" : "开始持续刷新 (2s)"}
          >
            {continuous ? <Pause size={14} /> : <Play size={14} />}
          </button>
          <button
            className="flex items-center justify-center w-6 h-6 rounded text-white/80 hover:text-red-400 hover:bg-red-400/20 transition-colors"
            onClick={handleClose}
            title="关闭"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Bottom-right resize handle */}
      <div
        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize pointer-events-auto"
        onMouseDown={onResizeStart}
      >
        <svg
          className="absolute bottom-0.5 right-0.5 text-sky-400/60"
          width="12"
          height="12"
          viewBox="0 0 12 12"
        >
          <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </div>
    </div>
  );
}
