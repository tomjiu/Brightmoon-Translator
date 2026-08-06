import { useCallback, useEffect, useRef, useState } from 'react';
import { listen, emit } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalSize } from '@tauri-apps/api/dpi';
import { Volume2, Copy, X, Pin, Loader2 } from 'lucide-react';
import { speakText } from '../services/tts';
import type { DictCard, TranslateResponse } from '../types';
import { useI18n } from '../i18n';

// Mirrors Rust overlay::translate_card event names.
const DATA_EVENT = 'translate-card-data';
const READY_EVENT = 'translate-card-ready';
const RENDERED_EVENT = 'translate-card-rendered';
const CLOSED_EVENT = 'translate-card-closed';
const EXPAND_REQUEST = 'translate-card-expand-request';
const EXPAND_RESULT = 'translate-card-expand-result';

type CardData =
  | {
      kind: 'mt';
      source: string;
      from: string;
      to: string;
      response: TranslateResponse;
      totalEngines: number;
    }
  | { kind: 'dict'; card: DictCard };

const LANG_LABEL: Record<string, string> = {
  auto: '自动',
  zh: '中文',
  en: '英语',
  ja: '日语',
  ko: '韩语',
  fr: '法语',
  de: '德语',
  es: '西语',
  ru: '俄语',
  it: '意语',
  ar: '阿语',
  th: '泰语',
  vi: '越语',
};

// Merge the full (all-engine) expand response into the card, keeping the engine
// that was shown first on top so the visible text doesn't jump.
function mergeResults(current: TranslateResponse, full: TranslateResponse): TranslateResponse {
  const results = full.results ?? [];
  const first = current.results?.[0]?.engine;
  if (first && results.length > 1 && results[0]?.engine !== first) {
    const head = results.find((r) => r.engine === first);
    if (head) {
      const rest = results.filter((r) => r.engine !== first);
      return { ...full, results: [head, ...rest] };
    }
  }
  return full;
}

/**
 * Floating translate card (Rust `translate-card` window). Rendered as a compact
 * Youdao-style popup: a slim title bar (drag / speak / copy / pin / close) over
 * either a multi-engine MT result or a dictionary entry.
 *
 * - `focus` cards (user-initiated 划词): close themselves on window blur.
 * - hover/dict cards: never steal focus, never auto-close on blur — the Rust
 *   `auto_watch` mouse-leave handles dismissal.
 * - The FE self-sizes the card via ResizeObserver → `set_size` (OcrRegionFrame
 *   pattern); the backend's initial size is only a placement estimate.
 */
export default function TranslateCard() {
  const { t } = useI18n();
  const tf = useCallback(
    (key: string, fallback: string) => {
      const result = t(key);
      return result === key ? fallback : result;
    },
    [t],
  );

  const win = getCurrentWindow();
  const [data, setData] = useState<CardData | null>(null);
  const [pinned, setPinned] = useState(false);
  const [busy, setBusy] = useState(false);
  const [expanding, setExpanding] = useState(false);
  const expandingRef = useRef(false);
  expandingRef.current = expanding;
  const [focusMode, setFocusMode] = useState(false);

  const lastNonce = useRef(0);
  const focusCard = useRef(false); // last event's `focus` flag
  const pinnedRef = useRef(false);
  const everFocused = useRef(false); // window has received focus this session
  const blurTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const roRef = useRef<ResizeObserver | null>(null);
  const pendingAck = useRef(0); // nonce awaiting a rendered-ack after sizing

  pinnedRef.current = pinned;

  const clearBlurTimer = useCallback(() => {
    if (blurTimer.current) {
      window.clearTimeout(blurTimer.current);
      blurTimer.current = null;
    }
  }, []);

  // Self-size: content → window. Clamp to backend's logical bounds (120-620 x 48-720).
  // After the FIRST sizing of a new payload, emit the render-ack so the backend
  // knows the content is applied and the window is at its real size before show.
  const applySize = useCallback(() => {
    const el = contentRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.min(620, Math.max(120, Math.ceil(r.width * dpr)));
    const h = Math.min(720, Math.max(48, Math.ceil(r.height * dpr)));
    const nonce = pendingAck.current;
    pendingAck.current = 0;
    const ack = () => {
      if (nonce > 0) void emit(RENDERED_EVENT, nonce).catch(() => undefined);
    };
    void win
      .setSize(new PhysicalSize(w, h))
      .then(ack)
      .catch(() => ack());
  }, [win]);

  const setContentEl = useCallback(
    (el: HTMLDivElement | null) => {
      contentRef.current = el;
      if (roRef.current) {
        roRef.current.disconnect();
        roRef.current = null;
      }
      if (el) {
        requestAnimationFrame(applySize);
        roRef.current = new ResizeObserver(applySize);
        roRef.current.observe(el);
      }
    },
    [applySize],
  );

  // Mount: register data listener, then signal ready (backend waits on it).
  useEffect(() => {
    let cancelled = false;
    let unData: (() => void) | undefined;
    void (async () => {
      try {
        unData = await listen<{
          nonce: number;
          focus: boolean;
          kind: 'mt' | 'dict';
          source?: string;
          from?: string;
          to?: string;
          response?: TranslateResponse;
          card?: DictCard;
          totalEngines?: number;
        }>(DATA_EVENT, (event) => {
          if (cancelled) return;
          const p = event.payload;
          if (!p || p.nonce === lastNonce.current) return; // dedupe cold-start re-emit
          // A new card is coming: cancel any pending blur-close and start a
          // fresh focus session (the backend hide→show cycle emits a blur that
          // must not close the upcoming card).
          clearBlurTimer();
          lastNonce.current = p.nonce;
          focusCard.current = !!p.focus;
          everFocused.current = false;
          setPinned(false);
          setExpanding(false);
          setFocusMode(!!p.focus);
          pendingAck.current = p.nonce;
          if (p.kind === 'mt' && p.response) {
            setData({
              kind: 'mt',
              source: p.source ?? '',
              from: p.from ?? 'auto',
              to: p.to ?? 'zh',
              response: p.response,
              totalEngines: p.totalEngines ?? 0,
            });
          } else if (p.kind === 'dict' && p.card) {
            setData({ kind: 'dict', card: p.card });
          }
          // Guarantee a sizing pass (and thus the rendered-ack) even when the
          // content dimensions don't change between cards (ResizeObserver may
          // not fire for same-size swaps).
          requestAnimationFrame(() => requestAnimationFrame(applySize));
        });
        await emit(READY_EVENT);
      } catch (e) {
        console.warn('[TranslateCard] listener setup failed', e);
      }
    })();
    return () => {
      cancelled = true;
      clearBlurTimer();
      unData?.();
    };
  }, [clearBlurTimer]);

  // Expand result: full (all-engine) translation for a quick card. Applied only
  // when the source text still matches the current card.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<{
      source: string;
      response: TranslateResponse;
    }>(EXPAND_RESULT, (event) => {
      const p = event.payload;
      setData((prev) => {
        if (!prev || prev.kind !== 'mt' || prev.source !== p.source) {
          return prev; // card dismissed or moved to another word — ignore
        }
        setExpanding(false);
        return {
          ...prev,
          response: mergeResults(prev.response, p.response),
          totalEngines: Math.max(prev.totalEngines, (p.response.results ?? []).length),
        };
      });
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      unlisten?.();
    };
  }, []);

  // Blur → close (focus cards only). Debounced so a blur emitted by the
  // backend hide→show reuse cycle is cancelled by the next data event instead
  // of hiding the freshly shown card. No-focus hover/dict cards survive any
  // focus change (auto_watch handles their dismissal).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void win
      .onFocusChanged(({ payload }) => {
        if (payload) {
          everFocused.current = true;
          clearBlurTimer();
          return;
        }
        if (!everFocused.current || !focusCard.current || pinnedRef.current) return;
        if (blurTimer.current) return;
        blurTimer.current = window.setTimeout(() => {
          blurTimer.current = null;
          void win.hide();
        }, 120);
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      clearBlurTimer();
      unlisten?.();
    };
  }, [win, clearBlurTimer]);

  // Esc hides (only meaningful for focus cards).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        void emit(CLOSED_EVENT).catch(() => undefined);
        void win.hide();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [win]);

  // Native OS drag via the title bar (buttons excluded).
  const onTitleMouseDown = useCallback(
    async (e: React.MouseEvent) => {
      if ((e.target as HTMLElement).closest('button')) return;
      e.preventDefault();
      try {
        await win.startDragging();
      } catch {
        /* ignore */
      }
    },
    [win],
  );

  const handleClose = useCallback(() => {
    clearBlurTimer();
    // Notify the backend so it suppresses hover re-presentation briefly
    // (otherwise the card can instantly reappear under the still-hovering cursor).
    void emit(CLOSED_EVENT).catch(() => undefined);
    void win.hide();
  }, [win, clearBlurTimer]);

  const handlePin = useCallback(() => {
    clearBlurTimer();
    setPinned((prev) => !prev);
  }, [clearBlurTimer]);

  // Expand a quick card: ask the backend to translate the remaining engines.
  const handleExpand = useCallback(() => {
    if (expandingRef.current) return;
    const m = mtRef.current;
    if (!m) return;
    setExpanding(true);
    void emit(EXPAND_REQUEST, { source: m.source, from: m.from, to: m.to }).catch(() =>
      setExpanding(false),
    );
  }, []);

  const copyText = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
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
      document.execCommand('copy');
      document.body.removeChild(ta);
    } catch {
      /* ignore */
    }
  }, []);

  const speak = useCallback(
    async (text: string, lang: string) => {
      setBusy(true);
      try {
        await speakText(text, lang);
      } catch {
        /* TTS may be unavailable */
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  // ---- Derived content ----
  const engineLabels = t('settings.enginePage.shortLabels') as unknown as
    | Record<string, string>
    | undefined;

  const mt = data?.kind === 'mt' ? data : null;
  const dict = data?.kind === 'dict' ? data.card : null;
  const mtRef = useRef(mt);
  mtRef.current = mt;

  const mtResults = mt?.response.results ?? [];
  const mtErrors = mt?.response.errors ?? [];
  const mtText = mtResults.map((r) => r.text).filter(Boolean).join('\n');
  const dictWord = dict?.word ?? '';
  const dictCopyText = dict
    ? [
        dict.word,
        dict.meanings
          ?.map((m) => `${m.pos} ${m.defs.join('；')}`)
          .join('\n') || '',
      ]
        .filter(Boolean)
        .join('\n')
    : '';

  const title = dict
    ? dictWord
    : mt
      ? `${LANG_LABEL[mt.from] ?? mt.from} → ${LANG_LABEL[mt.to] ?? mt.to}`
      : '';

  const btnBase =
    'w-6 h-5 rounded flex items-center justify-center transition-colors shrink-0';
  const btnIdle =
    'text-text-secondary hover:text-text-primary hover:bg-bg-tertiary';
  const btnActive = 'text-text-primary bg-bg-tertiary';

  return (
    <div
      ref={setContentEl}
      className="ui-glass ui-run-light flex flex-col overflow-hidden select-none text-text-primary"
      style={{
        minWidth: 140,
        maxWidth: 620,
        borderRadius: 8,
        boxShadow: 'var(--shadow-card-hover)',
        fontSize: 13,
        lineHeight: 1.45,
      }}
    >
      {/* Title bar */}
      <div
        className="h-7 px-1.5 flex items-center gap-0.5 border-b border-border bg-bg-tertiary/60 shrink-0"
        onMouseDown={onTitleMouseDown}
      >
        <span className="flex-1 truncate px-1 text-[11px] font-medium text-text-secondary select-none">
          {title}
        </span>
        {dict?.phonetic ? (
          <span className="text-[11px] text-text-secondary pr-1 select-none">
            {dict.phonetic}
          </span>
        ) : null}
        <button
          type="button"
          className={`${btnBase} ${btnIdle}`}
          disabled={busy}
          onClick={() => {
            if (dict) {
              void speak(dictWord, 'en');
            } else if (mt) {
              const text = mtText || mt.source;
              if (text) void speak(text, mt.to);
            }
          }}
          title={tf('common.speak', '朗读')}
        >
          {busy ? <Loader2 size={12} className="animate-spin" /> : <Volume2 size={12} />}
        </button>
        <button
          type="button"
          className={`${btnBase} ${btnIdle}`}
          onClick={() => {
            const text = dict ? dictCopyText : mtText || (mt?.source ?? '');
            if (text) void copyText(text);
          }}
          title={tf('common.copy', '复制')}
        >
          <Copy size={12} />
        </button>
        <button
          type="button"
          className={`${btnBase} ${pinned ? btnActive : btnIdle}`}
          onClick={handlePin}
          title={pinned ? tf('common.unpin', '取消钉住') : tf('common.pin', '钉住')}
        >
          <Pin size={12} />
        </button>
        <button
          type="button"
          className={`${btnBase} text-text-secondary hover:text-text-primary hover:bg-bg-tertiary`}
          onClick={handleClose}
          title={tf('common.close', '关闭')}
        >
          <X size={13} />
        </button>
      </div>

      {/* Body */}
      <div className="px-2.5 py-2 select-text">
        {mt ? (
          <div className="flex flex-col gap-1.5">
            {mt.source ? (
              <div className="text-[11px] text-text-secondary whitespace-pre-wrap break-words border-b border-border/50 pb-1.5">
                {mt.source}
              </div>
            ) : null}
            {mtResults.length > 0 ? (
              <div className="flex flex-col gap-1.5">
                {mtResults.map((r, i) => (
                  <div key={i} className="flex items-start gap-1.5">
                    <span className="mt-[2px] shrink-0 text-[10px] leading-4 px-1 rounded bg-bg-tertiary border border-border text-text-secondary whitespace-nowrap">
                      {engineLabels?.[r.engine] ?? r.engine}
                    </span>
                    <span className="whitespace-pre-wrap break-words text-text-primary">
                      {r.text}
                    </span>
                  </div>
                ))}
              </div>
            ) : null}
            {mtErrors.length > 0 && mtResults.length === 0 ? (
              <div className="text-text-secondary text-xs whitespace-pre-wrap break-words">
                {mtErrors.join('\n')}
              </div>
            ) : null}
            {/* Engine expand is only meaningful on user-initiated (focus) cards —
                hover previews stay one-engine quick results. */}
            {mt && focusMode && mtResults.length < (mt.totalEngines ?? 0) ? (
              <button
                type="button"
                disabled={expanding}
                onClick={handleExpand}
                className="mt-0.5 w-full h-6 rounded text-xs text-text-secondary hover:text-text-primary hover:bg-bg-tertiary border border-border/60 flex items-center justify-center gap-1 shrink-0"
              >
                {expanding ? (
                  <>
                    <Loader2 size={11} className="animate-spin" />
                    {tf('translateCard.expanding', '翻译其余引擎…')}
                  </>
                ) : (
                  tf('translateCard.expand', '翻译其余引擎') +
                  ` (${mt.totalEngines - mtResults.length})`
                )}
              </button>
            ) : null}
          </div>
        ) : dict ? (
          <div className="flex flex-col gap-2">
            {dict.meanings && dict.meanings.length > 0 ? (
              <div className="flex flex-col gap-1">
                {dict.meanings.map((m, i) => (
                  <div key={i} className="flex flex-col gap-0.5">
                    {m.pos ? (
                      <span className="text-[10px] font-semibold text-text-secondary">
                        {m.pos}
                      </span>
                    ) : null}
                    {m.defs.map((d, j) => (
                      <div key={j} className="flex gap-1.5 text-text-primary">
                        <span className="text-text-secondary shrink-0 select-none">
                          {i + 1}.{j + 1}
                        </span>
                        <span className="break-words">{d}</span>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            ) : null}
            {dict.examples && dict.examples.length > 0 ? (
              <div className="flex flex-col gap-1 border-t border-border/50 pt-1.5">
                {dict.examples.map((ex, i) => (
                  <div key={i} className="flex flex-col text-xs">
                    <span className="text-text-primary break-words">{ex.en}</span>
                    <span className="text-text-secondary break-words">{ex.zh}</span>
                  </div>
                ))}
              </div>
            ) : null}
            {dict.collins && dict.collins.length > 0 ? (
              <div className="flex flex-col gap-1.5 border-t border-border/50 pt-1.5">
                {dict.collins.map((c, i) => (
                  <div key={i} className="flex flex-col gap-0.5">
                    <span className="text-[10px] font-semibold text-text-secondary">
                      {c.pos} {c.posCn}
                    </span>
                    <span className="text-text-primary break-words text-xs">
                      {c.englishDef}
                    </span>
                    {c.examples.map((ex, j) => (
                      <div key={j} className="flex flex-col text-xs">
                        <span className="text-text-primary break-words">{ex.en}</span>
                        <span className="text-text-secondary break-words">{ex.zh}</span>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            ) : null}
            {dict.sources && dict.sources.length > 0 ? (
              <div className="text-[10px] text-text-secondary border-t border-border/50 pt-1.5 select-none">
                {dict.sources.join(' · ')}
              </div>
            ) : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}
