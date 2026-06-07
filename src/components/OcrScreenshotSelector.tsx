import { useEffect, useRef, useState } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { loadScreenshotSnapshot, type ScreenshotSnapshot } from '../services/ocr';
import { useI18n } from '../i18n';

interface Point {
  x: number;
  y: number;
}

interface SelectionPayload {
  left: number;
  top: number;
  width: number;
  height: number;
}

function normalizeRect(start: Point, end: Point) {
  const left = Math.min(start.x, end.x);
  const top = Math.min(start.y, end.y);
  return {
    left,
    top,
    width: Math.abs(end.x - start.x),
    height: Math.abs(end.y - start.y),
  };
}

function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

export default function OcrScreenshotSelector() {
  const { t } = useI18n();
  const imgRef = useRef<HTMLImageElement>(null);
  const [snapshot, setSnapshot] = useState<ScreenshotSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [start, setStart] = useState<Point | null>(null);
  const [current, setCurrent] = useState<Point | null>(null);

  useEffect(() => {
    loadScreenshotSnapshot()
      .then((snap) => {
        setSnapshot(snap);
        // Show window after snapshot is loaded to avoid black flash
        return getCurrentWindow().show();
      })
      .catch((err: unknown) => {
        setError(String(err));
        // Show window even on error so user can see the error message
        return getCurrentWindow().show();
      });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        void emitTo('main', 'ocr-screenshot-cancelled');
        void getCurrentWindow().close();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const finishSelection = async (end: Point) => {
    if (!start || !imgRef.current) return;

    const cssRect = normalizeRect(start, end);
    if (cssRect.width < 8 || cssRect.height < 8) {
      setStart(null);
      setCurrent(null);
      return;
    }

    const imageRect = imgRef.current.getBoundingClientRect();
    const left = clamp(cssRect.left - imageRect.left, 0, imageRect.width);
    const top = clamp(cssRect.top - imageRect.top, 0, imageRect.height);
    const right = clamp(cssRect.left + cssRect.width - imageRect.left, 0, imageRect.width);
    const bottom = clamp(cssRect.top + cssRect.height - imageRect.top, 0, imageRect.height);
    const scaleX = imgRef.current.naturalWidth / imageRect.width;
    const scaleY = imgRef.current.naturalHeight / imageRect.height;

    const payload: SelectionPayload = {
      left: Math.round(left * scaleX),
      top: Math.round(top * scaleY),
      width: Math.round((right - left) * scaleX),
      height: Math.round((bottom - top) * scaleY),
    };

    await emitTo('main', 'ocr-screenshot-selected', payload);
    await getCurrentWindow().close();
  };

  const selection = start && current ? normalizeRect(start, current) : null;

  return (
    <div
      className="fixed inset-0 z-50 bg-black text-white select-none cursor-crosshair overflow-hidden"
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        const point = { x: event.clientX, y: event.clientY };
        setStart(point);
        setCurrent(point);
      }}
      onMouseMove={(event) => {
        if (!start) return;
        setCurrent({ x: event.clientX, y: event.clientY });
      }}
      onMouseUp={(event) => finishSelection({ x: event.clientX, y: event.clientY })}
    >
      {snapshot && (
        <img
          ref={imgRef}
          src={snapshot.image}
          className="absolute inset-0 h-full w-full pointer-events-none"
          draggable={false}
          alt="screen snapshot"
        />
      )}

      <div className="absolute left-1/2 top-5 -translate-x-1/2 rounded-full bg-black/70 px-4 py-2 text-sm shadow-lg">
        {t('ocr.selectHint') || '拖拽选择要 OCR 翻译的区域，按 Esc 取消'}
      </div>

      {error && (
        <div className="absolute left-1/2 top-1/2 max-w-xl -translate-x-1/2 -translate-y-1/2 rounded-xl bg-red-600 p-4 shadow-xl">
          {error}
        </div>
      )}

      {selection && (
        <>
          <div className="absolute inset-0 bg-black/30 pointer-events-none" />
          <div
            className="absolute border-2 border-sky-400 bg-sky-400/10 shadow-[0_0_0_9999px_rgba(0,0,0,0.35)] pointer-events-none"
            style={{
              left: selection.left,
              top: selection.top,
              width: selection.width,
              height: selection.height,
            }}
          />
        </>
      )}
    </div>
  );
}
