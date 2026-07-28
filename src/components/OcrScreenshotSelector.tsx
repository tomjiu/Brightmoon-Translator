import { useEffect, useRef, useState } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
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

/**
 * pot-desktop pipeline (copied):
 * - Backend writes full-screen PNG to disk (Fast compression).
 * - FE uses convertFileSrc(path) — no full-screen base64 IPC.
 * - Show window only after img.onLoad (no long black frame).
 * Layout: img covers full client (object-fit:fill); FE must not re-pin window.
 */
export default function OcrScreenshotSelector() {
  const { t } = useI18n();
  const imgRef = useRef<HTMLImageElement>(null);
  const [imgUrl, setImgUrl] = useState('');
  const snapshotRef = useRef<ScreenshotSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [start, setStart] = useState<Point | null>(null);
  const [current, setCurrent] = useState<Point | null>(null);
  // Refs avoid pointerup racing setState (start still null on same gesture).
  const startRef = useRef<Point | null>(null);
  const currentRef = useRef<Point | null>(null);
  const finishingRef = useRef(false);
  const shownRef = useRef(false);

  const winShowReady = async () => {
    if (shownRef.current) return;
    shownRef.current = true;
    try {
      const win = getCurrentWindow();
      await win.show();
      await win.setFocus();
      // Main must not hide until freeze is on screen (avoids main-hide + hidden selector = void/black).
      await emitTo('main', 'ocr-screenshot-ready');
    } catch (e) {
      console.warn('[OCR selector] show failed', e);
    }
  };

  useEffect(() => {
    // Kill any leftover theme accent / focus blue in this window only
    const killBlue = document.createElement('style');
    killBlue.setAttribute('data-ocr-selector-neutral', '1');
    // Neutral dark chrome — pure #000 + empty img looked identical to "black screen" bug.
    killBlue.textContent = `
      html, body, #root { margin:0!important; padding:0!important; width:100%!important; height:100%!important;
        overflow:hidden!important; background:#111!important; outline:none!important; }
      * { outline: none !important; caret-color: transparent !important; }
      ::selection { background: rgba(255,255,255,0.18) !important; color: #fff !important; }
      *::-moz-focus-inner { border: 0 !important; }
    `;
    document.head.appendChild(killBlue);

    document.documentElement.style.cssText =
      'margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#111;outline:none;';
    document.body.style.cssText =
      'margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#111;outline:none;';
    const root = document.getElementById('root');
    if (root) {
      root.style.cssText =
        'margin:0;padding:0;width:100%;height:100%;overflow:hidden;background:#111;outline:none;';
    }

    const win = getCurrentWindow();

    // Do NOT re-set window position/size here on Windows.
    // Backend `force_hwnd_cover_physical` places OUTER at (-padL,-padT) so CLIENT
    // covers virtual desktop at (screenX,screenY). Calling setPosition(0,0) from FE
    // moves OUTER to origin and shifts the CLIENT by DWM chrome pads → frozen
    // preview / selection appear offset vs real desktop (esp. @ 125% DPI).
    // Multi-monitor negative origin is already handled in create_ocr_screenshot_selector.

    // pot: load file path → convertFileSrc → set img; show only onLoad.
    loadScreenshotSnapshot()
      .then(async (snap) => {
        snapshotRef.current = snap;
        const path = snap.imagePath;
        if (!path) throw new Error('snapshot path missing');
        // Do NOT append ?query — asset protocol often 404s and never paints freeze (black chrome).
        // Fragment only busts React/img cache identity without changing the asset path.
        const bust = `${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
        setImgUrl(`${convertFileSrc(path)}#${bust}`);
      })
      .catch(async (err: unknown) => {
        setError(String(err));
        try {
          await emitTo('main', 'ocr-screenshot-cancelled');
        } catch {
          /* ignore */
        }
        // Do not show() here — black chrome flash if snapshot failed after main hide.
        try {
          await win.close();
        } catch {
          /* ignore */
        }
      });

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        void emitTo('main', 'ocr-screenshot-cancelled');
        void getCurrentWindow().close();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      document.querySelector('style[data-ocr-selector-neutral]')?.remove();
    };
  }, []);

  const finishSelection = async (end: Point) => {
    if (finishingRef.current) return;
    const dragStart = startRef.current;
    const snap = snapshotRef.current;
    if (!dragStart || !imgRef.current || !snap) return;

    const cssRect = normalizeRect(dragStart, end);
    if (cssRect.width < 8 || cssRect.height < 8) {
      startRef.current = null;
      currentRef.current = null;
      setStart(null);
      setCurrent(null);
      return;
    }

    finishingRef.current = true;
    const img = imgRef.current;
    const nw = img.naturalWidth || snap.info.imageWidth;
    const nh = img.naturalHeight || snap.info.imageHeight;
    const dispW = img.clientWidth || img.getBoundingClientRect().width || window.innerWidth;
    const dispH = img.clientHeight || img.getBoundingClientRect().height || window.innerHeight;
    if (nw <= 0 || nh <= 0 || dispW < 1 || dispH < 1) {
      finishingRef.current = false;
      startRef.current = null;
      currentRef.current = null;
      setStart(null);
      setCurrent(null);
      return;
    }

    const dpiX = nw / dispW;
    const dpiY = nh / dispH;
    const rect = img.getBoundingClientRect();

    const x0 = Math.min(dragStart.x, end.x) - rect.left;
    const y0 = Math.min(dragStart.y, end.y) - rect.top;
    const x1 = Math.max(dragStart.x, end.x) - rect.left;
    const y1 = Math.max(dragStart.y, end.y) - rect.top;

    const left = Math.max(0, Math.floor(x0 * dpiX));
    const top = Math.max(0, Math.floor(y0 * dpiY));
    const right = Math.min(nw, Math.ceil(x1 * dpiX));
    const bottom = Math.min(nh, Math.ceil(y1 * dpiY));

    const payload: SelectionPayload = {
      left,
      top,
      width: Math.max(1, right - left),
      height: Math.max(1, bottom - top),
    };

    startRef.current = null;
    currentRef.current = null;
    setStart(null);
    setCurrent(null);

    try {
      // Emit only — main closes selector after region frame exists (less desktop flash).
      await emitTo('main', 'ocr-screenshot-selected', payload);
    } catch {
      finishingRef.current = false;
    }
  };

  const selection = start && current ? normalizeRect(start, current) : null;

  return (
    <div
      className="fixed inset-0 z-50 text-white select-none cursor-crosshair overflow-hidden outline-none"
      style={{ background: '#111', width: '100%', height: '100%', outline: 'none' }}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        finishingRef.current = false;
        try {
          event.currentTarget.setPointerCapture(event.pointerId);
        } catch {
          /* older webview */
        }
        const p = { x: event.clientX, y: event.clientY };
        startRef.current = p;
        currentRef.current = p;
        setStart(p);
        setCurrent(p);
      }}
      onPointerMove={(event) => {
        if (!startRef.current) return;
        const p = { x: event.clientX, y: event.clientY };
        currentRef.current = p;
        setCurrent(p);
      }}
      onPointerUp={(event) => {
        try {
          event.currentTarget.releasePointerCapture(event.pointerId);
        } catch {
          /* ignore */
        }
        void finishSelection({ x: event.clientX, y: event.clientY });
      }}
      onPointerCancel={() => {
        startRef.current = null;
        currentRef.current = null;
        setStart(null);
        setCurrent(null);
        finishingRef.current = false;
      }}
    >
      {imgUrl ? (
        <img
          ref={imgRef}
          src={imgUrl}
          className="fixed inset-0 select-none pointer-events-none"
          style={{
            margin: 0,
            padding: 0,
            border: 0,
            display: 'block',
            width: '100%',
            height: '100%',
            // Same aspect as full virtual screen → fill matches contain; keeps
            // clientWidth/Height == window CSS so natural/CSS scale is uniform.
            objectFit: 'fill',
          }}
          draggable={false}
          alt="screen snapshot"
          onLoad={() => {
            void winShowReady();
          }}
          onError={() => {
            console.error('[OCR selector] freeze image failed to load (asset protocol?)');
            setError('截图预览加载失败');
            void emitTo('main', 'ocr-screenshot-cancelled');
            void getCurrentWindow().close();
          }}
        />
      ) : (
        !error && (
          <div className="absolute inset-0 flex items-center justify-center text-white/80 text-sm z-10 pointer-events-none">
            正在加载截图… Esc 取消
          </div>
        )
      )}

      <div className="absolute left-1/2 top-5 -translate-x-1/2 rounded-full bg-black/85 px-4 py-2 text-sm shadow-lg z-10 pointer-events-none border border-white/40">
        <span className="text-white font-medium">
          {t('ocr.selectHint') || '拖拽选择区域，Esc 取消'}
        </span>
      </div>

      {error && (
        <div className="absolute left-1/2 top-1/2 max-w-xl -translate-x-1/2 -translate-y-1/2 rounded-xl bg-red-700/95 p-4 shadow-xl z-10">
          {error}
        </div>
      )}

      {selection && (
        <>
          {/* HARD neutral selection chrome — white only, never theme accent / sky blue */}
          <div
            className="absolute pointer-events-none z-[6]"
            style={{
              left: selection.left,
              top: selection.top,
              width: selection.width,
              height: selection.height,
              border: '2px solid #ffffff',
              boxShadow: '0 0 0 9999px rgba(0,0,0,0.5)',
              background: 'rgba(0,0,0,0.02)',
              borderRadius: 0,
              outline: 'none',
            }}
          />
        </>
      )}
    </div>
  );
}
