import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { captureScreen, ocrImage } from "../services/ocr";
import { useTranslateStore } from "../stores/translateStore";

// ─── Types ────────────────────────────────────────────────────────────────────

interface OcrRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface BoundWindow {
  hwnd: number;
  title: string;
  // Region offset relative to window top-left at bind time
  offset: { dx: number; dy: number; width: number; height: number };
  // Last known window rect
  lastRect: { x: number; y: number; width: number; height: number } | null;
}

interface OcrMonitorState {
  isMonitoring: boolean;
  paused: boolean;
  autoPaused: boolean;
  region: OcrRegion | null;
  lastText: string;
  lastGoodText: string; // preserved across OCR failures
  interval: number;
  clickThrough: boolean;
  pinned: boolean;
  boundWindow: BoundWindow | null;
  cycleCount: number;
  skipCount: number;
}

interface CycleDiag {
  captureMs: number;
  ocrMs: number;
  translateMs: number;
  textChanged: boolean;
  skipped: boolean;
  skipReason: string;
  qualityScore: number;
  textLen: number;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const SIMILARITY_THRESHOLD = 0.92; // text considered "same" above this
const MIN_TEXT_LENGTH = 2; // skip very short OCR results
const JITTER_WINDOW = 5; // number of recent texts to check for jitter
const MAX_CONSECUTIVE_EMPTY = 3; // after this many empty results, stop updating overlay
const FOLLOW_POLL_MS = 500;

// ─── Helpers ──────────────────────────────────────────────────────────────────

/** Simple character-level similarity (0..1). Fast, no allocation-heavy diff. */
function textSimilarity(a: string, b: string): number {
  if (a === b) return 1;
  if (!a || !b) return 0;
  const maxLen = Math.max(a.length, b.length);
  if (maxLen === 0) return 1;
  // Count matching characters in order (simplified LCS approximation)
  let matches = 0;
  let bi = 0;
  for (let ai = 0; ai < a.length && bi < b.length; ai++) {
    const idx = b.indexOf(a[ai], bi);
    if (idx !== -1) {
      matches++;
      bi = idx + 1;
    }
  }
  return matches / maxLen;
}

/** Check if text is likely OCR noise (random single chars, symbols). */
function isNoisyText(text: string): boolean {
  if (text.length < 3) return true;
  // High ratio of non-alphanumeric characters
  const alphanum = text.replace(/[^a-zA-Z0-9\u4e00-\u9fff]/g, "");
  if (alphanum.length / text.length < 0.3) return true;
  return false;
}

/** Check if recent texts form a jitter pattern (oscillating between similar results). */
function isJittery(recentTexts: string[]): boolean {
  if (recentTexts.length < JITTER_WINDOW) return false;
  const last = recentTexts.slice(-JITTER_WINDOW);
  // Check if texts alternate between 2-3 similar variants
  const unique = new Set(last);
  if (unique.size <= 2 && last.length >= JITTER_WINDOW) {
    return true;
  }
  return false;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

export function useOcrMonitor() {
  const [state, setState] = useState<OcrMonitorState>({
    isMonitoring: false,
    paused: false,
    autoPaused: false,
    region: null,
    lastText: "",
    lastGoodText: "",
    interval: 2000,
    clickThrough: false,
    pinned: false,
    boundWindow: null,
    cycleCount: 0,
    skipCount: 0,
  });

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastTextRef = useRef<string>("");
  const lastGoodTextRef = useRef<string>("");
  const busyRef = useRef(false);
  const noChangeCountRef = useRef(0);
  const baseIntervalRef = useRef(2000);
  const regionRef = useRef<OcrRegion | null>(null);
  const hwndRef = useRef<number>(0);
  const followTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const userPausedRef = useRef(false);
  const recentTextsRef = useRef<string[]>([]);
  const consecutiveEmptyRef = useRef(0);
  const cycleCountRef = useRef(0);
  const skipCountRef = useRef(0);
  const boundWindowRef = useRef<BoundWindow | null>(null);
  const overlayCreatedRef = useRef(false);
  const lastOverlayTextRef = useRef<string>("");
  const { setSourceText, translate } = useTranslateStore();

  // ── Adaptive delay ──

  const getAdaptiveDelay = useCallback(() => {
    const base = baseIntervalRef.current;
    const count = noChangeCountRef.current;
    if (count >= 10) return base * 4;
    if (count >= 5) return base * 2;
    return base;
  }, []);

  const scheduleNext = useCallback(
    (region: OcrRegion) => {
      const delay = getAdaptiveDelay();
      timerRef.current = setTimeout(() => {
        captureAndOcr(region);
      }, delay);
    },
    [getAdaptiveDelay]
  );

  // ── Quality check ──

  const checkQuality = useCallback(
    (text: string): { ok: boolean; reason: string; score: number } => {
      if (!text || text.trim().length === 0) {
        return { ok: false, reason: "empty", score: 0 };
      }
      const trimmed = text.trim();
      if (trimmed.length < MIN_TEXT_LENGTH) {
        return { ok: false, reason: "too_short", score: 0.1 };
      }
      if (isNoisyText(trimmed)) {
        return { ok: false, reason: "noisy", score: 0.2 };
      }
      // Check jitter against recent history
      const recent = recentTextsRef.current;
      if (isJittery([...recent, trimmed])) {
        return { ok: false, reason: "jitter", score: 0.3 };
      }
      // Check similarity to last text (debounce)
      const last = lastTextRef.current;
      if (last) {
        const sim = textSimilarity(trimmed, last);
        if (sim >= SIMILARITY_THRESHOLD) {
          return { ok: false, reason: "similar", score: sim };
        }
      }
      return { ok: true, reason: "", score: 1.0 };
    },
    []
  );

  // ── Main capture-OCR-translate cycle ──

  const captureAndOcr = useCallback(
    async (region: OcrRegion) => {
      if (busyRef.current) return;
      busyRef.current = true;
      cycleCountRef.current += 1;

      const diag: CycleDiag = {
        captureMs: 0,
        ocrMs: 0,
        translateMs: 0,
        textChanged: false,
        skipped: false,
        skipReason: "",
        qualityScore: 0,
        textLen: 0,
      };

      try {
        // 1. Capture
        const t0 = performance.now();
        const image = await captureScreen(
          region.x,
          region.y,
          region.width,
          region.height
        );
        diag.captureMs = performance.now() - t0;

        // 2. OCR
        const t1 = performance.now();
        const text = await ocrImage(image);
        diag.ocrMs = performance.now() - t1;
        diag.textLen = text.length;

        // 3. Quality check
        const quality = checkQuality(text);
        diag.qualityScore = quality.score;

        if (!quality.ok) {
          diag.skipped = true;
          diag.skipReason = quality.reason;
          skipCountRef.current += 1;

          // Track consecutive empty results
          if (quality.reason === "empty" || quality.reason === "too_short") {
            consecutiveEmptyRef.current += 1;
          }

          // For empty/noisy: keep last good text in overlay (don't clear)
          if (
            consecutiveEmptyRef.current >= MAX_CONSECUTIVE_EMPTY &&
            overlayCreatedRef.current
          ) {
            // Too many consecutive failures — log but don't destroy overlay
            console.log(
              `[OCR] ${consecutiveEmptyRef.current} consecutive empty results, keeping last overlay`
            );
          }
        } else {
          // Valid text
          consecutiveEmptyRef.current = 0;

          // Track in recent texts for jitter detection
          recentTextsRef.current.push(text.trim());
          if (recentTextsRef.current.length > JITTER_WINDOW * 2) {
            recentTextsRef.current = recentTextsRef.current.slice(-JITTER_WINDOW);
          }

          // Update lastText for display
          lastTextRef.current = text.trim();
          noChangeCountRef.current = 0;
          diag.textChanged = true;

          // Store as last good text
          lastGoodTextRef.current = text.trim();

          setState((prev) => ({
            ...prev,
            lastText: text.trim(),
            lastGoodText: text.trim(),
          }));

          // 4. Translate
          const t2 = performance.now();
          setSourceText(text.trim());
          await translate();
          diag.translateMs = performance.now() - t2;

          // 5. Update overlay (only if translated text changed)
          const result = useTranslateStore.getState().results[0];
          if (result) {
            const translatedText = result.text;
            if (translatedText !== lastOverlayTextRef.current || !overlayCreatedRef.current) {
              lastOverlayTextRef.current = translatedText;
              await invoke("update_overlay", {
                x: region.x + region.width + 10,
                y: region.y,
                width: 350,
                height: 200,
                text: translatedText,
                showControls: true,
              });
              overlayCreatedRef.current = true;
            }
          }
        }

        // Performance log
        const totalMs = diag.captureMs + diag.ocrMs + diag.translateMs;
        console.log(
          `[OCR] cycle=${cycleCountRef.current} capture=${diag.captureMs.toFixed(0)}ms ocr=${diag.ocrMs.toFixed(0)}ms translate=${diag.translateMs.toFixed(0)}ms total=${totalMs.toFixed(0)}ms changed=${diag.textChanged} skip=${diag.skipped ? diag.skipReason : "none"} quality=${diag.qualityScore.toFixed(2)} len=${diag.textLen}`
        );
      } catch (e) {
        console.error("[OCR] Monitor error:", e);
        diag.skipped = true;
        diag.skipReason = "error";
      } finally {
        busyRef.current = false;
        setState((prev) => ({
          ...prev,
          cycleCount: cycleCountRef.current,
          skipCount: skipCountRef.current,
        }));
        if (regionRef.current && !userPausedRef.current) {
          scheduleNext(region);
        }
      }
    },
    [setSourceText, translate, scheduleNext, checkQuality]
  );

  // ── Stop monitoring ──

  const stopMonitoring = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (followTimerRef.current) {
      clearInterval(followTimerRef.current);
      followTimerRef.current = null;
    }
    regionRef.current = null;
    hwndRef.current = 0;
    userPausedRef.current = false;
    recentTextsRef.current = [];
    consecutiveEmptyRef.current = 0;
    cycleCountRef.current = 0;
    skipCountRef.current = 0;
    overlayCreatedRef.current = false;
    lastOverlayTextRef.current = "";
    boundWindowRef.current = null;
    setState((prev) => ({
      ...prev,
      isMonitoring: false,
      paused: false,
      autoPaused: false,
      region: null,
      boundWindow: null,
      cycleCount: 0,
      skipCount: 0,
    }));
  }, []);

  // ── Pause / Resume ──

  const pauseMonitoring = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    userPausedRef.current = true;
    setState((prev) => ({ ...prev, paused: true }));
  }, []);

  const resumeMonitoring = useCallback(() => {
    const region = regionRef.current;
    if (!region) return;
    userPausedRef.current = false;
    setState((prev) => ({ ...prev, paused: false, autoPaused: false }));
    noChangeCountRef.current = 0;
    consecutiveEmptyRef.current = 0;
    captureAndOcr(region);
  }, [captureAndOcr]);

  // ── Auto-pause / Auto-resume ──

  const autoPause = useCallback(() => {
    if (userPausedRef.current) return;
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    setState((prev) => ({ ...prev, autoPaused: true }));
  }, []);

  const autoResume = useCallback(() => {
    if (userPausedRef.current) return;
    const region = regionRef.current;
    if (!region) return;
    setState((prev) => ({ ...prev, autoPaused: false }));
    noChangeCountRef.current = 0;
    consecutiveEmptyRef.current = 0;
    captureAndOcr(region);
  }, [captureAndOcr]);

  // ── Window binding ──

  const bindWindow = useCallback(
    async (region: OcrRegion) => {
      try {
        const hwnd = await invoke<number>("detect_foreground_hwnd");
        if (hwnd <= 0) return;

        const rect = await invoke<{
          x: number;
          y: number;
          width: number;
          height: number;
        } | null>("get_window_rect_cmd", { hwnd });

        if (!rect) return;

        // Get window title
        let title = "";
        try {
          title = await invoke<string>("get_window_title_cmd", { hwnd });
        } catch {
          title = `Window ${hwnd}`;
        }

        const bound: BoundWindow = {
          hwnd,
          title,
          offset: {
            dx: region.x - rect.x,
            dy: region.y - rect.y,
            width: region.width,
            height: region.height,
          },
          lastRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        };

        hwndRef.current = hwnd;
        boundWindowRef.current = bound;
        setState((prev) => ({ ...prev, boundWindow: bound }));

        // Start follow loop
        startFollowLoop(hwnd);
      } catch (e) {
        console.warn("[OCR] Failed to bind window:", e);
      }
    },
    []
  );

  const startFollowLoop = useCallback(
    (hwnd: number) => {
      if (followTimerRef.current) {
        clearInterval(followTimerRef.current);
      }

      followTimerRef.current = setInterval(async () => {
        try {
          const rect = await invoke<{
            x: number;
            y: number;
            width: number;
            height: number;
          } | null>("get_window_rect_cmd", { hwnd });

          if (!rect || !regionRef.current || !boundWindowRef.current) return;

          const bw = boundWindowRef.current;
          const lastRect = bw.lastRect;

          // Check if minimized (width/height == 0)
          if (rect.width === 0 && rect.height === 0) {
            if (!userPausedRef.current) {
              autoPause();
            }
            return;
          }

          // Check if window moved
          const shouldMove =
            !lastRect ||
            Math.abs(rect.x - lastRect.x) > 2 ||
            Math.abs(rect.y - lastRect.y) > 2;

          if (shouldMove) {
            // Compute new region from offset
            const newRegion: OcrRegion = {
              x: rect.x + bw.offset.dx,
              y: rect.y + bw.offset.dy,
              width: bw.offset.width,
              height: bw.offset.height,
            };
            regionRef.current = newRegion;
            boundWindowRef.current = {
              ...bw,
              lastRect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
            };
            setState((prev) => ({ ...prev, region: newRegion }));
          }

          // Auto-resume if was auto-paused and window is visible again
          if (
            state.autoPaused &&
            !userPausedRef.current &&
            rect.width > 0 &&
            rect.height > 0
          ) {
            autoResume();
          }
        } catch {
          // Window might be gone
        }
      }, FOLLOW_POLL_MS);
    },
    [autoPause, autoResume, state.autoPaused]
  );

  const unbindWindow = useCallback(() => {
    if (followTimerRef.current) {
      clearInterval(followTimerRef.current);
      followTimerRef.current = null;
    }
    hwndRef.current = 0;
    boundWindowRef.current = null;
    setState((prev) => ({ ...prev, boundWindow: null }));
  }, []);

  const rebindWindow = useCallback(
    async (region: OcrRegion) => {
      unbindWindow();
      await bindWindow(region);
    },
    [unbindWindow, bindWindow]
  );

  // ── Start monitoring ──

  const startMonitoring = useCallback(
    async (region: OcrRegion, interval: number = 2000) => {
      stopMonitoring();

      baseIntervalRef.current = interval;
      regionRef.current = region;
      noChangeCountRef.current = 0;
      userPausedRef.current = false;
      recentTextsRef.current = [];
      consecutiveEmptyRef.current = 0;
      cycleCountRef.current = 0;
      skipCountRef.current = 0;
      overlayCreatedRef.current = false;
      lastOverlayTextRef.current = "";

      setState({
        isMonitoring: true,
        paused: false,
        autoPaused: false,
        region,
        lastText: "",
        lastGoodText: "",
        interval,
        clickThrough: false,
        pinned: false,
        boundWindow: null,
        cycleCount: 0,
        skipCount: 0,
      });

      lastTextRef.current = "";
      lastGoodTextRef.current = "";

      // Bind to foreground window
      await bindWindow(region);

      // Initial capture
      captureAndOcr(region);
    },
    [captureAndOcr, stopMonitoring, bindWindow]
  );

  // ── Overlay controls ──

  const toggleClickThrough = useCallback(async () => {
    const newValue = !state.clickThrough;
    await invoke("set_overlay_click_through", { ignore: newValue });
    setState((prev) => ({ ...prev, clickThrough: newValue }));
  }, [state.clickThrough]);

  const togglePin = useCallback(async () => {
    const newValue = !state.pinned;
    if (newValue) {
      await invoke("pin_overlay");
    }
    setState((prev) => ({ ...prev, pinned: newValue }));
  }, [state.pinned]);

  // ── Visibility / Focus listeners ──

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.hidden) {
        autoPause();
      } else {
        autoResume();
      }
    };
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [autoPause, autoResume]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    const setupListener = async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const appWindow = getCurrentWindow();
        unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
          if (focused) {
            autoResume();
          } else {
            autoPause();
          }
        });
      } catch {
        // Ignore if not in Tauri context
      }
    };
    setupListener();
    return () => {
      if (unlisten) unlisten();
    };
  }, [autoPause, autoResume]);

  // ── Cleanup on unmount ──

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      if (followTimerRef.current) clearInterval(followTimerRef.current);
    };
  }, []);

  return {
    ...state,
    startMonitoring,
    stopMonitoring,
    pauseMonitoring,
    resumeMonitoring,
    toggleClickThrough,
    togglePin,
    bindWindow,
    unbindWindow,
    rebindWindow,
  };
}
