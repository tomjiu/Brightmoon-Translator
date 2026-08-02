import { useEffect, useRef, useState, useCallback, memo, useLayoutEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalSize } from '@tauri-apps/api/dpi';
import {
  RefreshCw,
  Play,
  Pause,
  X,
  Copy,
  Image,
  Languages,
  Pin,
  Link2,
  Download,
} from 'lucide-react';
import type { OcrLineResult } from '../services/ocr';
import { safeInvoke } from '../services/invoke';
import { useI18n } from '../i18n';
import { useConfigStore } from '../stores/configStore';
import {
  DEFAULT_ENGINE_ORDER,
} from '../pages/settings/engines/enginesMeta';
import {
  frameToCaptureRegion,
  ocrLineToCssRect,
  fitImageDisplayRect,
  OCR_TOOLBAR_HEIGHT_CSS,
  OCR_MIN_FRAME_WIDTH_CSS,
} from './ocrRegionGeometry';
import {
  OcrRegionEvents,
  OcrMainEvents,
  REGION_EVENTS_BY_ID,
  emitToMain as emitToMainBase,
  type OcrRegionUpdateData,
  type OcrRegionLoadingPayload,
  type OcrRegionErrorPayload,
  type OcrRegionEnabledPayload,
  type OcrRegionHintPayload,
  type OcrDisplayMode,
} from '../services/ocrRegionProtocol';

type OcrRegionData = OcrRegionUpdateData;

type DisplayMode = OcrDisplayMode;

// Use engine codes (zh not zh-CN) so translate/detect match Rust config defaults
const SUPPORTED_LANGS = ['auto', 'zh', 'en', 'ja', 'ko', 'fr', 'de', 'ru', 'es'];

const LANG_I18N_KEYS: Record<string, string> = {
  auto: 'common.autoDetect',
  zh: 'lang.zh',
  'zh-CN': 'lang.zh',
  en: 'lang.en',
  ja: 'lang.ja',
  ko: 'lang.ko',
  fr: 'lang.fr',
  de: 'lang.de',
  ru: 'lang.ru',
  es: 'lang.es',
};

/** Short labels for the narrow OCR toolbar selects (keep short so arrow fits). */
const LANG_FALLBACK: Record<string, string> = {
  auto: '自动',
  zh: '中文',
  'zh-CN': '中文',
  en: '英语',
  ja: '日语',
  ko: '韩语',
  fr: '法语',
  de: '德语',
  ru: '俄语',
  es: '西语',
};

// From ocrRegionGeometry — MUST match Rust OCR_TOOLBAR_CSS_PX / OCR_MIN_FRAME_CSS_W (I2/I3).
const TOOLBAR_HEIGHT = OCR_TOOLBAR_HEIGHT_CSS;
const MIN_FRAME_LOGICAL_W = OCR_MIN_FRAME_WIDTH_CSS;

// Memoized translation line — position uses image→CSS mapping, not raw DPR.
interface TranslationLineProps {
  line: OcrLineResult;
  translation: string;
  contentCssWidth: number;
  contentCssHeight: number;
  imagePixelWidth: number;
  imagePixelHeight: number;
  fallbackScale: number;
}

const TranslationLine = memo(
  ({
    line,
    translation,
    contentCssWidth,
    contentCssHeight,
    imagePixelWidth,
    imagePixelHeight,
    fallbackScale,
  }: TranslationLineProps) => {
    const rect = ocrLineToCssRect(
      line,
      contentCssWidth,
      contentCssHeight,
      imagePixelWidth,
      imagePixelHeight,
      fallbackScale,
    );
    const left = rect.x - 2;
    const top = rect.y - 1;
    const width = rect.width + 4;
    const height = Math.max(rect.height, 12);
    const fontSize = Math.max(11, Math.min(16, height * 0.72));

    return (
      <div
        className="absolute"
        style={{
          left,
          top,
          minWidth: width,
          maxWidth: Math.max(
            width,
            contentCssWidth > 0 ? Math.max(0, contentCssWidth - left - 4) : width,
          ),
        }}
      >
        {/* M5: near-opaque cover — the translation REPLACES the source text
            (kivio-style) instead of overlaying it translucent. Background is
            content-sized (not absolute inset-0) so long translations fully
            hide what's underneath. */}
        <div
          className="relative rounded-md"
          style={{
            minWidth: width,
            background: 'var(--ocr-overlay-bg-solid)',
            backdropFilter: 'blur(6px)',
            WebkitBackdropFilter: 'blur(6px)',
            boxShadow: '0 1px 6px rgba(0,0,0,0.35)',
          }}
        >
          <div
            className="font-medium whitespace-pre-wrap break-words px-1.5 py-0.5 select-text cursor-text"
            style={{
              minWidth: rect.width,
              fontSize: `${fontSize}px`,
              lineHeight: `${Math.max(height, fontSize + 2)}px`,
              color: 'var(--ocr-overlay-text)',
              textShadow: 'var(--ocr-overlay-text-shadow)',
              userSelect: 'text',
              WebkitUserSelect: 'text',
            }}
          >
            {translation}
          </div>
        </div>
      </div>
    );
  },
);

TranslationLine.displayName = 'TranslationLine';

const ENGINE_CFG_KEY: Record<string, string> = {
  google: 'google',
  youdao: 'youdao',
  baidu: 'baidu',
  deepl: 'deepl',
  deeplx: 'deeplx',
  microsoft: 'microsoft',
  yandex: 'yandex',
  offline: 'offline',
  caiyun: 'caiyun',
  tatoeba: 'tatoeba',
  baidu_web: 'baiduWeb',
  caiyun_web: 'caiyunWeb',
  volcengine_web: 'volcengineWeb',
  transmart: 'transmart',
  papago: 'papago',
};

function isEngineEnabled(
  engines: Record<string, { enabled?: boolean } | undefined>,
  id: string,
): boolean {
  if (id === 'llm') return true;
  const key = ENGINE_CFG_KEY[id] || id;
  return !!engines[key]?.enabled;
}

export default function OcrRegionFrame({ regionId }: { regionId?: string }) {
  const { t } = useI18n();
  const win = getCurrentWindow();
  // M3: regionId from URL. undefined / "default" → legacy single-frame behavior
  // (bare label + un-suffixed event names). Other ids → per-region frame.
  const rid = regionId ?? undefined;
  const isDefaultRegion = !rid || rid === 'default';
  const ev = useCallback(
    (base: string) => (isDefaultRegion ? base : `${base}-${rid}`),
    [isDefaultRegion, rid],
  );
  /**
   * M3: Emit a frame→main event. Event NAME stays the legacy base name (main
   * listens to base names only — simpler than per-region dynamic subscription);
   * routing happens via the `regionId` stamped into the payload. The legacy
   * default region is byte-identical (no stamping).
   */
  const emitMain = useCallback(
    (event: (typeof OcrMainEvents)[keyof typeof OcrMainEvents], payload?: unknown) => {
      if (isDefaultRegion) {
        return emitToMainBase(event, payload ?? null);
      }
      const stamped =
        payload && typeof payload === 'object'
          ? { ...payload, regionId: rid }
          : { regionId: rid };
      return emitToMainBase(event, stamped);
    },
    [isDefaultRegion, rid],
  );
  const engines = useConfigStore((s) => s.config.engines);
  const engineOrder = useConfigStore((s) => s.config.engineOrder);

  const getLangName = useCallback(
    (code: string) => {
      const key = LANG_I18N_KEYS[code];
      const result = key ? t(key) : '';
      return result && result !== key ? result : LANG_FALLBACK[code] || code;
    },
    [t],
  );
  // tf = translate with fallback: returns fallback if t() returns the key itself (meaning not found)
  const tf = useCallback(
    (key: string, fallback: string) => {
      const result = t(key);
      return result === key ? fallback : result;
    },
    [t],
  );
  const [data, setData] = useState<OcrRegionData | null>(null);
  // M4: engine dropdown shows THIS region's engine (data.engine from main) —
  // fall back to the global primary enabled engine when not set.
  const primaryEngineId = (() => {
    if (data?.engine) return data.engine;
    const order =
      engineOrder && engineOrder.length > 0 ? engineOrder : (DEFAULT_ENGINE_ORDER as string[]);
    const eng = engines as unknown as Record<string, { enabled?: boolean } | undefined>;
    return order.find((id) => isEngineEnabled(eng, id)) || order[0] || 'youdao';
  })();
  const [continuous, setContinuous] = useState(false); // Default OFF — continuous hide/OCR/show flickers
  const [displayMode, setDisplayMode] = useState<DisplayMode>('translation');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pinned, setPinned] = useState(true); // always-on-top pin
  const [followWindow, setFollowWindow] = useState(false); // track target window move

  const resizeStart = useRef({ x: 0, y: 0, width: 0, height: 0 });
  const [sourceLang, setSourceLang] = useState('auto');
  // Placeholder until first payload; app default is zh (do not hardcode en — confuses UI before data arrives)
  const [targetLang, setTargetLang] = useState('zh');
  // Prefer image natural size for line layout; DPR only as geometry fallback (I5).
  // Re-read on resize so multi-monitor DPI moves stay correct (I2).
  const [scaleFactor, setScaleFactor] = useState(() => window.devicePixelRatio || 1);
  const sourceLangRef = useRef(sourceLang);
  const targetLangRef = useRef(targetLang);
  const dataRef = useRef<OcrRegionData | null>(null);
  const displayModeRef = useRef<DisplayMode>(displayMode);
  const toolbarRef = useRef<HTMLDivElement>(null);
  const [contentSize, setContentSize] = useState({ w: 0, h: 0 });
  const [imageSize, setImageSize] = useState({ w: 0, h: 0 });

  useEffect(() => {
    sourceLangRef.current = sourceLang;
  }, [sourceLang]);
  useEffect(() => {
    targetLangRef.current = targetLang;
  }, [targetLang]);
  useEffect(() => {
    dataRef.current = data;
  }, [data]);
  useEffect(() => {
    displayModeRef.current = displayMode;
  }, [displayMode]);

  useEffect(() => {
    const syncDpr = () => setScaleFactor(window.devicePixelRatio || 1);
    syncDpr();
    const mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    const onChange = () => syncDpr();
    mq.addEventListener('change', onChange);
    window.addEventListener('resize', syncDpr);
    return () => {
      mq.removeEventListener('change', onChange);
      window.removeEventListener('resize', syncDpr);
    };
  }, []);

  // Change detection: store previous OCR text for comparison
  const prevSourceTextRef = useRef<string>('');

  // Screenshot cache to prevent re-loading - use separate state that doesn't trigger re-render
  const screenshotUrlRef = useRef<string | null>(null);
  const screenshotImgRef = useRef<HTMLImageElement | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  /**
   * Screen capture rect under the frame. When min-width expands the window wider
   * than the OCR image, image is centered — report the painted content origin,
   * not the full chrome width (keeps drag/refresh aligned with desktop text).
   */
  const getCaptureRegion = useCallback(async () => {
    // Prefer INNER (client) geometry — outer is shifted by DWM chrome after force_hwnd_cover.
    // Capture math is client-aligned with create_ocr_region_frame.
    let pos = await win.outerPosition();
    let size = await win.outerSize();
    try {
      pos = await win.innerPosition();
      size = await win.innerSize();
    } catch {
      /* outer fallback */
    }
    // Prefer OS scale factor for the window's monitor (more accurate than JS DPR alone).
    let scale = scaleFactor;
    try {
      scale = await win.scaleFactor();
    } catch {
      /* keep devicePixelRatio fallback */
    }
    // Fixed toolbar height so capture rect matches create_ocr_region_frame (I2).
    let region = frameToCaptureRegion(
      { x: pos.x, y: pos.y, width: size.width, height: size.height },
      TOOLBAR_HEIGHT,
      scale,
    );
    // Min-width chrome is wider than the true crop; image is centered in content CSS.
    // Report the painted image rect (not full chrome) so drag/refresh stay on text.
    const iw = imageSize.w;
    const ih = imageSize.h;
    const cw = contentSize.w;
    const ch = contentSize.h;
    if (iw > 0 && ih > 0 && cw > 0 && ch > 0) {
      const painted = fitImageDisplayRect(cw, ch, iw, ih);
      if (painted.width > 0 && painted.width + 1 < cw) {
        const leftPadPhys = painted.x * scale;
        const paintWPhys = painted.width * scale;
        region = {
          x: Math.round(region.x + leftPadPhys),
          y: region.y,
          width: Math.round(paintWPhys),
          height: region.height,
        };
      }
    }
    return region;
  }, [scaleFactor, win, imageSize.w, imageSize.h, contentSize.w, contentSize.h]);

  // Dark translucent chrome (Youdao-like glass). Window itself is transparent.
  // Strip any global accent blue that might paint borders/focus rings.
  useEffect(() => {
    const s = document.createElement('style');
    s.setAttribute('data-ocr-region-neutral', '1');
    s.textContent =
      'html,body,#root{background:transparent!important;margin:0!important;outline:none!important}' +
      '*{outline:none!important}' +
      '::selection{background:rgba(255,255,255,0.15)!important}';
    document.head.appendChild(s);
    document.documentElement.style.background = 'transparent';
    document.body.style.backgroundColor = 'transparent';
    document.body.style.margin = '0';
    const root = document.getElementById('root');
    if (root) root.style.backgroundColor = 'transparent';
    return () => {
      s.remove();
    };
  }, []);

  // Content size for I5: bootstrap + observe (not only when `data` arrives).
  useLayoutEffect(() => {
    const bootW = window.innerWidth;
    const bootH = Math.max(1, window.innerHeight - TOOLBAR_HEIGHT);
    if (bootW > 0 && bootH > 0) setContentSize({ w: bootW, h: bootH });

    const contentEl = () =>
      containerRef.current?.querySelector('[data-ocr-content]') as HTMLElement | null;
    const apply = () => {
      const el = contentEl();
      if (!el) {
        const w = window.innerWidth;
        const h = Math.max(1, window.innerHeight - TOOLBAR_HEIGHT);
        if (w > 0 && h > 0) setContentSize({ w, h });
        return;
      }
      const r = el.getBoundingClientRect();
      setContentSize({ w: r.width, h: r.height });
    };
    apply();
    const el = contentEl();
    if (!el) {
      window.addEventListener('resize', apply);
      return () => window.removeEventListener('resize', apply);
    }
    const observer = new ResizeObserver(apply);
    observer.observe(el);
    window.addEventListener('resize', apply);
    return () => {
      observer.disconnect();
      window.removeEventListener('resize', apply);
    };
  }, []);

  // Focus window so keyboard shortcuts work (handlers registered after copyToClipboard).
  useEffect(() => {
    void win.setFocus().catch(() => undefined);
  }, [win]);

  // Listen for data updates; emit ready only AFTER listeners are registered (selection waits on it).
  useEffect(() => {
    let cancelled = false;
    const unlisteners: Array<() => void> = [];
    const receivedDataRef = { current: false };

    let dataTimeoutId: ReturnType<typeof setTimeout> | null = null;
    const armDataTimeout = () => {
      if (dataTimeoutId) window.clearTimeout(dataTimeoutId);
      dataTimeoutId = window.setTimeout(() => {
        if (cancelled || receivedDataRef.current) return;
        console.warn('[OcrRegionFrame] Data timeout after 30 seconds');
        setError(tf('ocrRegion.dataTimeout', '等待数据超时，请点击重试'));
        setLoading(false);
      }, 30000);
    };

    const applySessionReset = () => {
      // Soft reset: clear OCR text state but keep freeze screenshot if already painted.
      // Hard-clearing img made the frame flash empty ("clicked once and gone").
      receivedDataRef.current = false;
      armDataTimeout();
      setContinuous(false);
      setLoading(true);
      setError(null);
      setFollowWindow(false);
      setDisplayMode('translation');
      setActionHint(null);
      prevSourceTextRef.current = '';
      setData((prev) =>
        prev?.screenshot
          ? {
              screenshot: prev.screenshot,
              sourceText: '',
              translatedText: '',
              ocrLines: [],
              lineTranslations: [],
              sourceLang: prev.sourceLang,
              targetLang: prev.targetLang,
              refreshIntervalMs: prev.refreshIntervalMs,
            }
          : null,
      );
      void emitMain(OcrMainEvents.sessionResetAck, null).catch(() => undefined);
    };

    armDataTimeout();

    // Register all critical listeners, then emit ready once (no partial-listen race).
    void (async () => {
      try {
        // P0 fix: main pings via OcrRegionEvents.pingReady (same base name for
        // every region). M3 renamed the ready event for non-default ids, but
        // main never emits ocr-region-ready-{id} — listening on pingReady
        // restores the handshake for both default and per-region frames.
        const unPing = await listen(OcrRegionEvents.pingReady, () => {
          if (cancelled) return;
          void emitMain(OcrMainEvents.frameReady, null).catch(() => undefined);
        });
        if (cancelled) {
          unPing();
          return;
        }
        unlisteners.push(unPing);

        const unReset = await listen(ev(OcrRegionEvents.sessionReset), () => {
          if (cancelled) return;
          applySessionReset();
        });
        if (cancelled) {
          unReset();
          return;
        }
        unlisteners.push(unReset);

        const unData = await listen<OcrRegionData>(REGION_EVENTS_BY_ID.text(rid ?? 'default'), (event) => {
          if (cancelled) return;
          const d = event.payload;

          const textChanged = prevSourceTextRef.current !== d.sourceText;
          const screenshotChanged = !!d.screenshot && d.screenshot !== screenshotUrlRef.current;
          const linesChanged =
            !dataRef.current ||
            dataRef.current.ocrLines.length !== (d.ocrLines.length ?? 0) ||
            d.ocrLines.some((l, i) => {
              const p = dataRef.current?.ocrLines[i];
              return (
                !p || p.x !== l.x || p.y !== l.y || p.width !== l.width || p.height !== l.height
              );
            });
          const transChanged =
            d.translatedText !== dataRef.current?.translatedText ||
            (d.lineTranslations.length ?? 0) !== (dataRef.current.lineTranslations.length ?? 0);
          if (
            !textChanged &&
            prevSourceTextRef.current &&
            !screenshotChanged &&
            !linesChanged &&
            !transChanged
          ) {
            if (d.imageWidth && d.imageHeight && d.imageWidth > 0 && d.imageHeight > 0) {
              setImageSize({ w: d.imageWidth, h: d.imageHeight });
            }
            receivedDataRef.current = true;
            setLoading(false);
            return;
          }

          if (textChanged) {
            prevSourceTextRef.current = d.sourceText;
          }
          receivedDataRef.current = true;

          if (d.screenshot && d.screenshot !== screenshotUrlRef.current) {
            screenshotUrlRef.current = d.screenshot;
            if (screenshotImgRef.current) {
              screenshotImgRef.current.src = d.screenshot;
            }
          }

          if (d.imageWidth && d.imageHeight && d.imageWidth > 0 && d.imageHeight > 0) {
            setImageSize({ w: d.imageWidth, h: d.imageHeight });
          }
          setData(d);
          setLoading(false);
          if (!d.keepError) setError(null);
          setSourceLang(d.sourceLang);
          setTargetLang(d.targetLang);
        });
        if (cancelled) {
          unData();
          return;
        }
        unlisteners.push(unData);

        void emitMain(OcrMainEvents.frameReady, null).catch(() => undefined);
      } catch (e) {
        console.warn('[OcrRegionFrame] listener setup failed', e);
      }
    })();

    return () => {
      cancelled = true;
      if (dataTimeoutId) window.clearTimeout(dataTimeoutId);
      unlisteners.forEach((fn) => fn());
    };
  }, [tf]);

  // Emit initial position only for follow offset sync — parent must NOT re-OCR on this
  // and must NOT adopt min-width-expanded width (see OcrScreenshotTranslator handler).
  // Fire after first paint (rAF×2) instead of fixed 400ms delay.
  useEffect(() => {
    let cancelled = false;
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(() => {
        if (cancelled) return;
        void getCaptureRegion()
          .then((r) => emitMain(OcrMainEvents.positionChanged, r))
          .catch(() => undefined);
      });
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
    };
  }, [getCaptureRegion]);

  // Sync follow button if main window fails to bind target window
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<OcrRegionEnabledPayload>(ev(OcrRegionEvents.followState), (event) => {
      if (cancelled) return;
      setFollowWindow(event.payload.enabled);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // OCR / translate errors from main window (keep frame open)
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<OcrRegionErrorPayload>(REGION_EVENTS_BY_ID.error(rid ?? 'default'), (event) => {
      if (cancelled) return;
      setError(event.payload.message || tf('ocr.noTextRecognized', 'OCR 没有识别到文本'));
      setLoading(false);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [tf]);

  // Soft busy during refresh/continuous grab (frame stays visible — spinner only)
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<OcrRegionLoadingPayload>(ev(OcrRegionEvents.loading), (event) => {
      if (cancelled) return;
      if (event.payload.loading) {
        setError(null);
        setLoading(true);
      } else {
        setLoading(false);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Main may pause continuous (e.g. target minimized) — keep toolbar in sync
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<OcrRegionEnabledPayload>(ev(OcrRegionEvents.continuousState), (event) => {
      if (cancelled) return;
      setContinuous(!!event.payload.enabled);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const pauseParentRefresh = useCallback(() => {
    if (continuous) {
      void emitMain(OcrMainEvents.continuous, { enabled: false });
    }
  }, [continuous]);

  const restoreParentRefresh = useCallback(() => {
    if (continuous) {
      void emitMain(OcrMainEvents.continuous, { enabled: true });
    }
  }, [continuous]);

  // ---- Native OS drag: toolbar chrome only (not content — that ate clicks / felt like "gone") ----
  const onToolbarMouseDown = async (e: React.MouseEvent) => {
    if (
      (e.target as HTMLElement).closest('button') ||
      (e.target as HTMLElement).closest('select')
    ) {
      return;
    }
    e.preventDefault();
    pauseParentRefresh();
    try {
      await win.startDragging();
      await emitMain(OcrMainEvents.positionChanged, await getCaptureRegion());
    } catch {
      /* ignore */
    } finally {
      restoreParentRefresh();
    }
  };

  // ---- Resize from corner handle ----
  const resizing = useRef(false);
  const onResizeStart = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    resizing.current = true;
    pauseParentRefresh();
    const size = await win.outerSize();
    resizeStart.current = { x: e.screenX, y: e.screenY, width: size.width, height: size.height };
    const onMove = (ev: MouseEvent) => {
      if (!resizing.current) return;
      const dx = ev.screenX - resizeStart.current.x;
      const dy = ev.screenY - resizeStart.current.y;
      const minW = Math.round(MIN_FRAME_LOGICAL_W * scaleFactor);
      const minH = Math.round((TOOLBAR_HEIGHT + 48) * scaleFactor);
      void win.setSize(
        new PhysicalSize(
          Math.max(minW, resizeStart.current.width + dx),
          Math.max(minH, resizeStart.current.height + dy),
        ),
      );
    };
    const onUp = async () => {
      resizing.current = false;
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
      try {
        await emitMain(OcrMainEvents.sizeChanged, await getCaptureRegion());
      } catch {
        /* ignore */
      } finally {
        restoreParentRefresh();
      }
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  };

  const [actionHint, setActionHint] = useState<{ text: string; error?: boolean } | null>(null);
  const actionHintTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flashHint = useCallback((msg: string, error = false) => {
    setActionHint({ text: msg, error });
    if (actionHintTimer.current) window.clearTimeout(actionHintTimer.current);
    actionHintTimer.current = window.setTimeout(() => setActionHint(null), 1400);
  }, []);

  useEffect(() => {
    return () => {
      if (actionHintTimer.current) window.clearTimeout(actionHintTimer.current);
    };
  }, []);

  // Non-blocking hints (same-lang, etc.) — toolbar flash, not red error panel
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen<OcrRegionHintPayload>(ev(OcrRegionEvents.hint), (event) => {
      if (cancelled) return;
      const msg = event.payload.message;
      if (msg) flashHint(msg);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [flashHint]);

  // ---- Button handlers ----
  const handleRefresh = useCallback(() => {
    setError(null);
    setLoading(true);
    flashHint(tf('ocrRegion.refreshing', '刷新中…'));
    void emitMain(OcrMainEvents.refresh, null);
  }, [flashHint, tf]);

  const handleToggleContinuous = useCallback(() => {
    const next = !continuous;
    setContinuous(next);
    void emitMain(OcrMainEvents.continuous, { enabled: next });
    flashHint(
      next ? tf('ocrRegion.watchOn', '监视已开启') : tf('ocrRegion.watchOff', '监视已关闭'),
    );
  }, [continuous, flashHint, tf]);

  const handleClose = useCallback(() => {
    // Just emit the close event — the main window will close this window
    // via the Rust `close_ocr_region_frame` command, then show itself.
    // This avoids both windows being visible at the same time.
    void emitMain(OcrMainEvents.close, null);
  }, []);

  const handleTogglePin = useCallback(async () => {
    const next = !pinned;
    try {
      await win.setAlwaysOnTop(next);
      setPinned(next);
      flashHint(next ? tf('ocrRegion.pinned', '已置顶') : tf('ocrRegion.unpinned', '取消置顶'));
    } catch {
      // ignore — window may be closing
    }
  }, [pinned, win, flashHint, tf]);

  const handleToggleFollow = useCallback(() => {
    const next = !followWindow;
    setFollowWindow(next);
    void emitMain(OcrMainEvents.follow, { enabled: next });
    flashHint(next ? tf('ocrRegion.followOn', '跟随窗口') : tf('ocrRegion.followOff', '取消跟随'));
  }, [followWindow, flashHint, tf]);

  const copyToClipboard = useCallback(
    async (text: string) => {
      if (!text) {
        flashHint(tf('ocrRegion.nothingToCopy', '无可复制内容'), true);
        return;
      }
      try {
        await navigator.clipboard.writeText(text);
        flashHint(tf('common.copied', '已复制'));
        return;
      } catch {
        // fall through
      }
      try {
        const ta = document.createElement('textarea');
        ta.value = text;
        ta.style.position = 'fixed';
        ta.style.left = '-9999px';
        document.body.appendChild(ta);
        ta.select();
        const ok = document.execCommand('copy');
        document.body.removeChild(ta);
        if (ok) {
          flashHint(tf('common.copied', '已复制'));
        } else {
          flashHint(tf('ocrRegion.copyFailed', '复制失败'), true);
        }
      } catch {
        console.warn('[OcrRegionFrame] copy failed');
        flashHint(tf('ocrRegion.copyFailed', '复制失败'), true);
      }
    },
    [flashHint, tf],
  );

  // Esc closes; Ctrl/Cmd+C copies; R refreshes (ignore when focus in select/input).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        void emitMain(OcrMainEvents.close, null);
        return;
      }
      const tag = (e.target as HTMLElement).tagName;
      const typing =
        tag === 'INPUT' ||
        tag === 'TEXTAREA' ||
        tag === 'SELECT' ||
        (e.target as HTMLElement).isContentEditable;
      if ((e.ctrlKey || e.metaKey) && (e.key === 'c' || e.key === 'C')) {
        const sel = window.getSelection()?.toString();
        if (sel && sel.length > 0) return;
        e.preventDefault();
        const d = dataRef.current;
        if (!d) return;
        const text =
          displayModeRef.current === 'source' || displayModeRef.current === 'image'
            ? d.sourceText
            : d.translatedText || d.sourceText;
        void copyToClipboard(text);
        return;
      }
      if (!typing && !e.ctrlKey && !e.metaKey && !e.altKey && (e.key === 'r' || e.key === 'R')) {
        e.preventDefault();
        handleRefresh();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [copyToClipboard, handleRefresh]);

  const handleCopyScreenshot = useCallback(async () => {
    if (!data?.screenshot) {
      flashHint(tf('ocrRegion.needOcr', '识别完成后可用'), true);
      return;
    }
    try {
      const blob = await (await fetch(data.screenshot)).blob();
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
      flashHint(tf('ocrRegion.copiedImage', '截图已复制'));
    } catch {
      flashHint(tf('ocrRegion.copyImageFailed', '截图复制失败'), true);
    }
  }, [data, flashHint, tf]);

  /** Save region crop PNG to disk (dialog). */
  const handleSaveScreenshot = useCallback(async () => {
    if (!data?.screenshot) {
      flashHint(tf('ocrRegion.needOcr', '识别完成后可用'), true);
      return;
    }
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({
        defaultPath: `ocr-region-${Date.now()}.png`,
        filters: [{ name: 'PNG', extensions: ['png'] }],
      });
      if (!path) {
        flashHint(tf('ocrRegion.saveCancelled', '已取消保存'));
        return;
      }
      const [, saveErr] = await safeInvoke(
        'write_file_base64',
        { filePath: path, base64Data: data.screenshot },
        { silent: true },
      );
      if (saveErr) {
        console.warn('[OcrRegionFrame] save screenshot failed', saveErr);
        flashHint(tf('ocrRegion.saveFailed', '保存失败'), true);
        return;
      }
      flashHint(tf('ocrRegion.savedImage', '已保存'));
    } catch (e) {
      console.warn('[OcrRegionFrame] save screenshot failed', e);
      flashHint(tf('ocrRegion.saveFailed', '保存失败'), true);
    }
  }, [data, flashHint, tf]);

  const handleLangChange = useCallback((type: 'source' | 'target', value: string) => {
    if (type === 'source') {
      setSourceLang(value);
      sourceLangRef.current = value;
    } else {
      setTargetLang(value);
      targetLangRef.current = value;
    }
    void emitMain(OcrMainEvents.langChange, {
      sourceLang: type === 'source' ? value : sourceLangRef.current,
      targetLang: type === 'target' ? value : targetLangRef.current,
    });
  }, []);

  const handleEngineSelect = useCallback(
    (engineId: string) => {
      if (!engineId) return;
      // M4: engine dropdown = THIS region's engine choice (per-region). Does
      // not touch the global primary order / enabled flags.
      void emitMain(OcrMainEvents.engineChange, {
        engineId,
        enabled: true,
        promote: false,
        perRegion: true,
      });
      flashHint(tf('ocrRegion.engineSwitched', '已切换引擎'));
    },
    [emitMain, flashHint, tf],
  );

  const handleEngineToggleEnabled = useCallback(
    (engineId: string, enabled: boolean) => {
      // Global engine enable/disable (management), not per-region.
      void emitMain(OcrMainEvents.engineChange, {
        engineId,
        enabled,
        promote: enabled,
        perRegion: false,
      });
      flashHint(
        enabled ? tf('ocrRegion.engineOn', '引擎已启用') : tf('ocrRegion.engineOff', '引擎已关闭'),
      );
    },
    [emitMain, flashHint, tf],
  );

  // ---- Compute text area bounds from OCR lines (CSS space) ----
  const textAreaBounds = useCallback(() => {
    if (!data?.ocrLines.length) return null;
    const validLines = data.ocrLines.filter((l) => l.width > 0 && l.height > 0);
    if (!validLines.length) return null;
    let minX = Infinity,
      minY = Infinity,
      maxR = -Infinity,
      maxB = -Infinity;
    for (const l of validLines) {
      const r = ocrLineToCssRect(
        l,
        contentSize.w,
        contentSize.h,
        imageSize.w,
        imageSize.h,
        scaleFactor,
      );
      if (r.x < minX) minX = r.x;
      if (r.y < minY) minY = r.y;
      if (r.x + r.width > maxR) maxR = r.x + r.width;
      if (r.y + r.height > maxB) maxB = r.y + r.height;
    }
    return { x: minX, y: minY, width: maxR - minX, height: maxB - minY };
  }, [data, contentSize.w, contentSize.h, imageSize.w, imageSize.h, scaleFactor]);

  const bounds = textAreaBounds();
  const refreshIntervalLabel = `${Math.round(((data?.refreshIntervalMs ?? 2000) / 1000) * 10) / 10}s`;
  // Action availability — never shrink buttons; disable when data missing
  const hasSource = Boolean(data?.sourceText.trim());
  const hasTarget = Boolean(data?.translatedText.trim());
  const hasShot = Boolean(data?.screenshot);
  const canCopySource = hasSource;
  const canCopyTarget = hasTarget || hasSource;
  const canCopyShot = hasShot;
  const canSaveShot = hasShot;
  const canToggleDisplay = hasSource || hasTarget;
  const canRefresh = !loading;
  const btnBase =
    'flex items-center justify-center w-5 h-5 rounded transition-colors flex-shrink-0 shrink-0';
  const btnIdle =
    'text-[var(--ocr-overlay-text-soft)] hover:text-[var(--ocr-overlay-text)] hover:bg-[var(--ocr-overlay-btn-bg-hover)]';
  const btnOff = 'text-[var(--ocr-overlay-text-muted)] cursor-not-allowed';

  return (
    <div
      ref={containerRef}
      className="fixed inset-0 select-none outline-none overflow-visible"
      style={{
        // Do NOT use overflow:hidden on the root — it clips toolbar icons on narrow frames.
        minWidth: MIN_FRAME_LOGICAL_W,
        background: 'var(--ocr-overlay-bg)',
        // Neutral gray border only — never sky/accent blue
        border: '1px solid var(--ocr-overlay-border)',
        borderRadius: 10,
        boxShadow: 'var(--ocr-overlay-shadow)',
        outline: 'none',
        WebkitTapHighlightColor: 'transparent',
      }}
    >
      {/* Full toolbar strip: drag only here (content clicks must not startDragging). */}
      <div
        ref={toolbarRef}
        className="absolute top-0 left-0 right-0 z-50 px-1.5 flex flex-nowrap items-center gap-0.5 text-xs overflow-x-auto overflow-y-hidden [&::-webkit-scrollbar]:hidden"
        onMouseDown={onToolbarMouseDown}
        style={{
          height: `${TOOLBAR_HEIGHT}px`,
          minHeight: `${TOOLBAR_HEIGHT}px`,
          maxHeight: `${TOOLBAR_HEIGHT}px`,
          minWidth: MIN_FRAME_LOGICAL_W,
          background: 'var(--ocr-overlay-bg-solid)',
          backdropFilter: 'blur(12px)',
          WebkitBackdropFilter: 'blur(12px)',
          borderBottom: '1px solid var(--ocr-overlay-border-soft)',
          borderTopLeftRadius: 10,
          borderTopRightRadius: 10,
        }}
      >
        {/* Display mode toggle - compact */}
        <button
          type="button"
          disabled={!canToggleDisplay}
          className={`px-1.5 py-0.5 rounded font-medium transition-colors text-[11px] flex-shrink-0 shrink-0 ${
            !canToggleDisplay
              ? btnOff
              : displayMode === 'translation'
                ? 'bg-[var(--ocr-overlay-btn-bg)] text-[var(--ocr-overlay-text)]'
                : 'text-[var(--ocr-overlay-text-soft)] hover:text-[var(--ocr-overlay-text)] hover:bg-[var(--ocr-overlay-btn-bg-hover)]'
          }`}
          onClick={() =>
            setDisplayMode(
              displayMode === 'translation'
                ? 'source'
                : displayMode === 'source'
                  ? 'image'
                  : 'translation',
            )
          }
          title={tf('ocrRegion.toggleDisplay', '切换原图/原文/译文')}
        >
          {displayMode === 'translation' ? '译' : displayMode === 'source' ? '原' : '图'}
        </button>

        <span className="w-px h-3 bg-[var(--ocr-overlay-border-soft)] flex-shrink-0" />

        {/* Copy buttons - icon only */}
        <button
          type="button"
          disabled={!canCopySource}
          className={`${btnBase} ${canCopySource ? btnIdle : btnOff}`}
          onClick={() => canCopySource && data && copyToClipboard(data.sourceText)}
          title={
            canCopySource
              ? tf('ocrRegion.copySource', '复制原文')
              : tf('ocrRegion.needOcr', '识别完成后可用')
          }
          aria-label={tf('ocrRegion.copySource', '复制原文')}
        >
          <Copy size={11} />
        </button>

        <button
          type="button"
          disabled={!canCopyTarget}
          className={`${btnBase} ${canCopyTarget ? 'text-[var(--ocr-overlay-text)] hover:text-[var(--ocr-overlay-text)] hover:bg-[var(--ocr-overlay-btn-bg-hover)]' : btnOff}`}
          onClick={() =>
            canCopyTarget && data && copyToClipboard(data.translatedText || data.sourceText)
          }
          title={
            canCopyTarget
              ? tf('ocrRegion.copyTarget', '复制译文')
              : tf('ocrRegion.needOcr', '识别完成后可用')
          }
          aria-label={tf('ocrRegion.copyTarget', '复制译文')}
        >
          <span className="text-[9px] font-semibold leading-none">译</span>
        </button>

        <button
          type="button"
          disabled={!canCopyShot}
          className={`${btnBase} ${canCopyShot ? btnIdle : btnOff}`}
          onClick={() => {
            if (canCopyShot) void handleCopyScreenshot();
          }}
          title={
            canCopyShot
              ? tf('ocrRegion.copyScreenshot', '复制截图')
              : tf('ocrRegion.needOcr', '识别完成后可用')
          }
        >
          <Image size={11} />
        </button>

        <button
          type="button"
          disabled={!canSaveShot}
          className={`${btnBase} ${canSaveShot ? btnIdle : btnOff}`}
          onClick={() => {
            if (canSaveShot) void handleSaveScreenshot();
          }}
          title={
            canSaveShot
              ? tf('ocrRegion.saveScreenshot', '保存截图')
              : tf('ocrRegion.needOcr', '识别完成后可用')
          }
          aria-label={tf('ocrRegion.saveScreenshot', '保存截图')}
        >
          <Download size={11} />
        </button>

        <span className="w-px h-3 bg-[var(--ocr-overlay-border-soft)] flex-shrink-0" />

        {/* Language always shown — window min-width fits full toolbar (no hide-on-narrow). */}
        <Languages size={10} className="text-[var(--ocr-overlay-text-muted)] flex-shrink-0" />
        <select
          className="bg-[var(--ocr-overlay-input-bg)] text-[var(--ocr-overlay-text)] rounded border border-[var(--ocr-overlay-border-soft)] pl-1 pr-4 py-0.5 text-[10px] cursor-pointer flex-shrink-0 shrink-0 appearance-auto"
          style={{
            width: '4.5rem',
            minWidth: '4.5rem',
            maxWidth: '5.5rem',
            // Room for native dropdown arrow (WebView often overlays it on text)
            paddingRight: '1.1rem',
            textOverflow: 'ellipsis',
          }}
          value={sourceLang}
          onChange={(e) => handleLangChange('source', e.target.value)}
          title={getLangName(sourceLang)}
        >
          {SUPPORTED_LANGS.map((l) => (
            <option key={l} value={l}>
              {getLangName(l)}
            </option>
          ))}
        </select>
        <span className="text-[var(--ocr-overlay-text-muted)] text-[10px] flex-shrink-0">→</span>
        <select
          className="bg-[var(--ocr-overlay-input-bg)] text-[var(--ocr-overlay-text)] rounded border border-[var(--ocr-overlay-border-soft)] pl-1 pr-4 py-0.5 text-[10px] cursor-pointer flex-shrink-0 shrink-0 appearance-auto"
          style={{
            width: '4rem',
            minWidth: '4rem',
            maxWidth: '5rem',
            paddingRight: '1.1rem',
            textOverflow: 'ellipsis',
          }}
          value={targetLang}
          onChange={(e) => handleLangChange('target', e.target.value)}
          title={getLangName(targetLang)}
        >
          {SUPPORTED_LANGS.filter((l) => l !== 'auto').map((l) => (
            <option key={l} value={l}>
              {getLangName(l)}
            </option>
          ))}
        </select>
        <span className="w-px h-3 bg-[var(--ocr-overlay-border-soft)] flex-shrink-0" />

        {/* Engine switch + enable (primary order) */}
        <select
          className="bg-[var(--ocr-overlay-input-bg)] text-[var(--ocr-overlay-text)] rounded border border-[var(--ocr-overlay-border-soft)] pl-1 pr-4 py-0.5 text-[10px] cursor-pointer flex-shrink-0 shrink-0 appearance-auto"
          style={{
            width: '5.2rem',
            minWidth: '5.2rem',
            maxWidth: '6.5rem',
            paddingRight: '1.1rem',
            textOverflow: 'ellipsis',
          }}
          value={primaryEngineId}
          onChange={(e) => handleEngineSelect(e.target.value)}
          title={tf('ocrRegion.engine', '翻译引擎')}
        >
          {(engineOrder && engineOrder.length > 0
            ? engineOrder
            : (DEFAULT_ENGINE_ORDER as string[])
          ).map((id) => {
            // S3-3: engine short label via i18n (previously derived from
            // enginesMeta.nameZh with a zh-only regex strip — broke for
            // non-Chinese locales and coupled OCR UI to a dead meta field).
            const labels = t('settings.enginePage.shortLabels') as unknown as
              | Record<string, string>
              | undefined;
            const label = labels?.[String(id)] || String(id);
            const eng = engines as unknown as Record<string, { enabled?: boolean } | undefined>;
            const on = isEngineEnabled(eng, id);
            return (
              <option key={id} value={id}>
                {on ? '● ' : '○ '}
                {label}
              </option>
            );
          })}
        </select>
        <button
          type="button"
          className={`${btnBase} ${
            isEngineEnabled(
              engines as unknown as Record<string, { enabled?: boolean } | undefined>,
              primaryEngineId,
            )
              ? 'text-emerald-300 bg-emerald-400/15'
              : btnIdle
          }`}
          onClick={() => {
            const eng = engines as unknown as Record<string, { enabled?: boolean } | undefined>;
            const on = isEngineEnabled(eng, primaryEngineId);
            handleEngineToggleEnabled(primaryEngineId, !on);
          }}
          title={
            isEngineEnabled(
              engines as unknown as Record<string, { enabled?: boolean } | undefined>,
              primaryEngineId,
            )
              ? tf('ocrRegion.disableEngine', '关闭当前引擎')
              : tf('ocrRegion.enableEngine', '启用当前引擎')
          }
        >
          <span className="text-[9px] font-semibold leading-none">机</span>
        </button>
        <span className="w-px h-3 bg-[var(--ocr-overlay-border-soft)] flex-shrink-0" />

        {/* Pin always-on-top */}
        <button
          type="button"
          className={`${btnBase} ${pinned ? 'text-amber-300 bg-amber-400/15' : btnIdle}`}
          onClick={() => void handleTogglePin()}
          title={pinned ? tf('ocrRegion.unpin', '取消钉住') : tf('ocrRegion.pin', '钉住置顶')}
        >
          <Pin size={11} />
        </button>

        {/* Follow target window */}
        <button
          type="button"
          className={`${btnBase} ${followWindow ? 'text-emerald-300 bg-emerald-400/15' : btnIdle}`}
          onClick={handleToggleFollow}
          title={
            followWindow
              ? tf('ocrRegion.unfollow', '停止跟随窗口')
              : tf('ocrRegion.follow', '跟随目标窗口（先点目标窗口再点此按钮）')
          }
        >
          <Link2 size={11} />
        </button>

        {/* Auto refresh toggle - icon only */}
        <button
          type="button"
          className={`${btnBase} ${continuous ? 'text-[var(--ocr-overlay-text)] bg-[var(--ocr-overlay-btn-bg)]' : btnIdle}`}
          onClick={handleToggleContinuous}
          title={
            continuous
              ? `${tf('ocrRegion.pauseWatch', '暂停监视')} (${refreshIntervalLabel})`
              : tf('ocrRegion.startWatch', '监视（内容变化才译）')
          }
        >
          {continuous ? <Pause size={11} /> : <Play size={11} />}
        </button>

        {/* Manual refresh */}
        <button
          type="button"
          disabled={!canRefresh}
          className={`${btnBase} ${canRefresh ? btnIdle : btnOff}`}
          onClick={() => {
            if (canRefresh) handleRefresh();
          }}
          title={
            canRefresh
              ? tf('ocrRegion.refreshNow', '立即刷新')
              : tf('ocrRegion.refreshing', '刷新中…')
          }
        >
          <RefreshCw size={11} className={loading ? 'animate-spin' : undefined} />
        </button>

        <span className="w-2 flex-shrink-0" />

        {/* Close — always available */}
        <button
          type="button"
          className={`${btnBase} text-[var(--ocr-overlay-text-soft)] hover:text-red-300 hover:bg-red-400/15`}
          onClick={handleClose}
          title={tf('common.close', '关闭')}
        >
          <X size={12} />
        </button>

        {actionHint ? (
          <span
            className={`ml-1 text-[10px] pointer-events-none whitespace-nowrap flex-shrink-0 ${
              actionHint.error ? 'text-red-300/95' : 'text-emerald-300/90'
            }`}
          >
            {actionHint.text}
          </span>
        ) : null}
      </div>

      {/* ---- Content Area (translation overlays) ---- */}
      <div
        data-ocr-content
        className="absolute overflow-hidden"
        style={{ top: TOOLBAR_HEIGHT, left: 0, right: 0, bottom: 0 }}
        onDoubleClick={(e) => {
          // Double-click empty chrome refreshes; ignore when selecting overlay text.
          if ((e.target as HTMLElement).closest('button, select, .select-text')) return;
          handleRefresh();
        }}
      >
        {/* Loading: keep previous overlays visible under a light veil (manual refresh). */}
        {loading && !error && (
          <div className="absolute inset-0 flex items-center justify-center pointer-events-none z-20 bg-black/25">
            <div className="text-[var(--ocr-overlay-text)] text-xs animate-pulse px-2 py-1 rounded bg-[var(--ocr-overlay-input-bg)]">
              {tf('ocrRegion.recognizing', '正在识别文本...')}
            </div>
          </div>
        )}

        {/* Error state — keep prior overlays under a dim panel when possible */}
        {error && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 pointer-events-auto z-30 bg-black/40">
            <div className="text-red-200 text-sm px-3 text-center max-w-[90%]">{error}</div>
            <button
              className="px-3 py-1.5 bg-[var(--ocr-overlay-btn-bg)] text-[var(--ocr-overlay-text)] rounded text-xs hover:bg-[var(--ocr-overlay-btn-bg-hover)] transition-colors"
              onClick={() => {
                setError(null);
                setLoading(true);
                handleRefresh();
              }}
            >
              {tf('common.retry', '重试')}
            </button>
          </div>
        )}

        {/* Captured screenshot as dimmed background - use ref to prevent re-render */}
        {(() => {
          const painted = fitImageDisplayRect(
            contentSize.w,
            contentSize.h,
            imageSize.w,
            imageSize.h,
          );
          return (
            <img
              ref={screenshotImgRef}
              src={screenshotUrlRef.current || ''}
              className="absolute pointer-events-none select-none"
              style={{
                opacity: displayMode === 'image' ? 1 : 0.18,
                left: painted.x,
                top: painted.y,
                // Same contain + horizontal center as ocrLineToCssRect.
                width: painted.width > 0 ? painted.width : undefined,
                height: painted.height > 0 ? painted.height : undefined,
                maxWidth: '100%',
                maxHeight: '100%',
                objectFit: 'fill',
              }}
              alt=""
              draggable={false}
              onLoad={(e) => {
                const img = e.currentTarget;
                if (img.naturalWidth > 0 && img.naturalHeight > 0) {
                  setImageSize({ w: img.naturalWidth, h: img.naturalHeight });
                }
              }}
            />
          );
        })()}

        {/* Translation overlay at source text position — immersive line-by-line replacement */}
        {data && displayMode === 'translation' && data.ocrLines.length > 0 && (
          <div className="absolute inset-0">
            {data.ocrLines.map((line, i) =>
              line.width > 0 && line.height > 0 ? (
                <TranslationLine
                  key={`${line.x}-${line.y}-${line.width}-${line.height}-${i}`}
                  line={line}
                  translation={data.lineTranslations[i] || line.text}
                  contentCssWidth={contentSize.w}
                  contentCssHeight={contentSize.h}
                  imagePixelWidth={imageSize.w}
                  imagePixelHeight={imageSize.h}
                  fallbackScale={scaleFactor}
                />
              ) : null,
            )}
          </div>
        )}

        {/* Fallback: full translation when no usable line boxes (zero-size / empty geometry). */}
        {data &&
          displayMode === 'translation' &&
          data.translatedText &&
          data.translatedText !== data.sourceText &&
          (!data.ocrLines.some((l) => l.width > 0 && l.height > 0) ||
            !data.lineTranslations.length) &&
          bounds && (
            <div
              className="absolute"
              style={{
                left: Math.max(0, bounds.x - 4),
                top: Math.max(0, bounds.y - 2),
                maxWidth: Math.min(bounds.width + 8, Math.max(80, contentSize.w - bounds.x - 8)),
              }}
            >
              <div
                className="rounded-md px-2 py-1"
                style={{
                  background: 'var(--ocr-overlay-bg-strong)',
                  backdropFilter: 'blur(6px)',
                  WebkitBackdropFilter: 'blur(6px)',
                }}
              >
                <div
                  className="text-xs leading-normal font-medium select-text cursor-text"
                  style={{ color: 'var(--ocr-overlay-text)' }}
                >
                  {data.translatedText}
                </div>
              </div>
            </div>
          )}

        {/* Source text overlay */}
        {data && displayMode === 'source' && data.ocrLines.length > 0 && (
          <div className="absolute inset-0">
            {data.ocrLines.map((line, i) => {
              if (!(line.width > 0 && line.height > 0)) return null;
              const r = ocrLineToCssRect(
                line,
                contentSize.w,
                contentSize.h,
                imageSize.w,
                imageSize.h,
                scaleFactor,
              );
              return (
                <div
                  key={`${line.x}-${line.y}-${line.width}-${line.height}-${i}`}
                  className="absolute rounded px-1 text-xs leading-tight whitespace-nowrap select-text cursor-text"
                  style={{
                    left: r.x,
                    top: r.y - 2,
                    maxWidth: r.width + 4,
                    background: 'var(--ocr-overlay-bg-toolbar)',
                    color: 'var(--ocr-overlay-text-soft)',
                  }}
                >
                  {line.text}
                </div>
              );
            })}
          </div>
        )}

        {/* Bottom-right resize handle */}
        <div
          className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize pointer-events-auto z-10"
          onMouseDown={onResizeStart}
          title={tf('ocrRegion.resize', '拖动调整大小')}
          role="separator"
          aria-label={tf('ocrRegion.resize', '拖动调整大小')}
        >
          <svg
            className="absolute bottom-0.5 right-0.5 text-[var(--ocr-overlay-text-muted)]"
            width="12"
            height="12"
            viewBox="0 0 12 12"
          >
            <path d="M11 1L1 11M11 5L5 11M11 9L9 11" stroke="currentColor" strokeWidth="1.5" />
          </svg>
        </div>
      </div>
    </div>
  );
}
