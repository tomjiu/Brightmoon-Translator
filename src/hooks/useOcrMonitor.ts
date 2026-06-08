import { useState, useRef, useCallback, useEffect } from "react";
import { safeInvoke } from "../services/invoke";
import { captureScreen, ocrImage } from "../services/ocr";
import { useTranslateStore } from "../stores/translateStore";
import { useConfigStore } from "../stores/configStore";
import { checkQuality, JITTER_WINDOW, MAX_CONSECUTIVE_EMPTY } from "./ocrQuality";
import { WindowBindingManager } from "./ocrWindowBinding";
import type { BoundWindow } from "./ocrWindowBinding";
import { OverlaySyncManager } from "./ocrOverlaySync";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface OcrRegion {
  x: number;
  y: number;
  width: number;
  height: number;
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
  lastDiag: CycleDiag | null;
}

export interface CycleDiag {
  captureMs: number;
  ocrMs: number;
  translateMs: number;
  textChanged: boolean;
  skipped: boolean;
  skipReason: string;
  qualityScore: number;
  textLen: number;
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
    lastDiag: null,
  });

  // ── Refs ──
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastTextRef = useRef<string>("");
  const lastGoodTextRef = useRef<string>("");
  const busyRef = useRef(false);
  const noChangeCountRef = useRef(0);
  const baseIntervalRef = useRef(2000);
  const regionRef = useRef<OcrRegion | null>(null);
  const userPausedRef = useRef(false);
  const autoPausedRef = useRef(false);
  const recentTextsRef = useRef<string[]>([]);
  const consecutiveEmptyRef = useRef(0);
  const cycleCountRef = useRef(0);
  const skipCountRef = useRef(0);
  const overlayRef = useRef<OverlaySyncManager>(new OverlaySyncManager());
  const { setSourceText, translate } = useTranslateStore();

  // Window binding manager — created once, callbacks updated via ref pattern
  const autoPauseRef = useRef<() => void>(() => {});
  const autoResumeRef = useRef<() => void>(() => {});

  const windowBindingRef = useRef<WindowBindingManager>(
    new WindowBindingManager({
      onRegionUpdate: (newRegion) => {
        regionRef.current = newRegion;
        setState((prev) => ({ ...prev, region: newRegion }));
      },
      onWindowMinimized: () => {
        autoPauseRef.current();
      },
      onWindowRestored: () => {
        autoResumeRef.current();
      },
      onOverlayPositionSync: (x, y) => {
        overlayRef.current.updatePosition(x, y);
      },
    })
  );

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
        // Always use regionRef.current to get the latest region (window may have moved)
        captureAndOcr(regionRef.current || region);
      }, delay);
    },
    [getAdaptiveDelay]
  );

  // ── Main capture-OCR-translate cycle ──

  const captureAndOcr = useCallback(
    async (region: OcrRegion) => {
      if (busyRef.current) return;
      // Use the latest region from ref in case window moved during the cycle
      const currentRegion = regionRef.current || region;
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
          currentRegion.x,
          currentRegion.y,
          currentRegion.width,
          currentRegion.height
        );
        diag.captureMs = performance.now() - t0;

        // 2. OCR
        const t1 = performance.now();
        const text = await ocrImage(image);
        diag.ocrMs = performance.now() - t1;
        diag.textLen = text.length;

        // 3. Quality check (delegated to ocrQuality module)
        const quality = checkQuality(text, lastTextRef.current, recentTextsRef.current);
        diag.qualityScore = quality.score;

        if (!quality.ok) {
          diag.skipped = true;
          diag.skipReason = quality.reason;
          skipCountRef.current += 1;

          if (quality.reason === "similar" || quality.reason === "jitter" || quality.reason === "noisy") {
            noChangeCountRef.current += 1;
          }

          if (quality.reason === "empty" || quality.reason === "too_short") {
            consecutiveEmptyRef.current += 1;
            noChangeCountRef.current += 1;
          }

          if (
            consecutiveEmptyRef.current >= MAX_CONSECUTIVE_EMPTY &&
            overlayRef.current.isCreated()
          ) {
            // Keep last overlay after many consecutive empty results
          }
        } else {
          // Valid text
          consecutiveEmptyRef.current = 0;

          // Track in recent texts for jitter detection
          recentTextsRef.current.push(text.trim());
          if (recentTextsRef.current.length > JITTER_WINDOW * 2) {
            recentTextsRef.current = recentTextsRef.current.slice(-JITTER_WINDOW);
          }

          lastTextRef.current = text.trim();
          noChangeCountRef.current = 0;
          diag.textChanged = true;
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

          // 5. Update overlay (delegated to ocrOverlaySync module)
          const result = useTranslateStore.getState().results[0];
          if (result) {
            await overlayRef.current.update(currentRegion, result.text);
          }
        }

        // Performance diagnostics available in diag object if needed
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
          lastDiag: diag,
        }));
        if (regionRef.current && !userPausedRef.current) {
          scheduleNext(regionRef.current);
        }
      }
    },
    [setSourceText, translate, scheduleNext]
  );

  // ── Stop monitoring ──

  const stopMonitoring = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    windowBindingRef.current.unbind();
    regionRef.current = null;
    userPausedRef.current = false;
    autoPausedRef.current = false;
    recentTextsRef.current = [];
    consecutiveEmptyRef.current = 0;
    cycleCountRef.current = 0;
    skipCountRef.current = 0;
    overlayRef.current.reset();
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
    autoPausedRef.current = true;
    setState((prev) => ({ ...prev, autoPaused: true }));
  }, []);

  const autoResume = useCallback(() => {
    if (userPausedRef.current) return;
    const region = regionRef.current;
    if (!region) return;
    autoPausedRef.current = false;
    setState((prev) => ({ ...prev, autoPaused: false }));
    noChangeCountRef.current = 0;
    consecutiveEmptyRef.current = 0;
    captureAndOcr(region);
  }, [captureAndOcr]);

  // Keep refs in sync for the WindowBindingManager callbacks
  autoPauseRef.current = autoPause;
  autoResumeRef.current = autoResume;

  // ── Window binding (delegated to ocrWindowBinding module) ──

  const bindWindow = useCallback(
    async (region: OcrRegion) => {
      const bound = await windowBindingRef.current.bind(region);
      if (bound) {
        windowBindingRef.current.setRegionRef(regionRef.current);
        setState((prev) => ({ ...prev, boundWindow: bound }));
      }
    },
    []
  );

  const unbindWindow = useCallback(() => {
    windowBindingRef.current.unbind();
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
    async (region: OcrRegion, interval?: number) => {
      stopMonitoring();

      const config = useConfigStore.getState().config;
      const resolvedInterval = interval ?? config.ocrInterval ?? 2000;
      const clickThrough = config.ocrClickThrough ?? false;

      baseIntervalRef.current = resolvedInterval;
      regionRef.current = region;
      noChangeCountRef.current = 0;
      userPausedRef.current = false;
      recentTextsRef.current = [];
      consecutiveEmptyRef.current = 0;
      cycleCountRef.current = 0;
      skipCountRef.current = 0;

      setState({
        isMonitoring: true,
        paused: false,
        autoPaused: false,
        region,
        lastText: "",
        lastGoodText: "",
        interval: resolvedInterval,
        clickThrough,
        pinned: false,
        boundWindow: null,
        cycleCount: 0,
        skipCount: 0,
        lastDiag: null,
      });

      lastTextRef.current = "";
      lastGoodTextRef.current = "";

      if (clickThrough) {
        const [, err] = await safeInvoke("set_overlay_click_through", { ignore: true }, { silent: true });
        if (err) {
          console.warn("[OCR] Failed to set click-through:", err);
        }
      }

      const autoBind = config.ocrAutoBindWindow ?? true;
      if (autoBind) {
        await bindWindow(region);
      }

      captureAndOcr(region);
    },
    [captureAndOcr, stopMonitoring, bindWindow]
  );

  // ── Overlay controls ──

  const toggleClickThrough = useCallback(async () => {
    const newValue = !state.clickThrough;
    await safeInvoke("set_overlay_click_through", { ignore: newValue }, { silent: true });
    setState((prev) => ({ ...prev, clickThrough: newValue }));
    useConfigStore.getState().updateConfig((prev) => ({ ...prev, ocrClickThrough: newValue }));
  }, [state.clickThrough]);

  const togglePin = useCallback(async () => {
    const [result] = await safeInvoke<boolean>("pin_overlay", undefined, { silent: true });
    if (result !== null) {
      setState((prev) => ({ ...prev, pinned: result }));
    }
  }, []);

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
    let cancelled = false;
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
        if (cancelled && unlisten) {
          unlisten();
          unlisten = null;
        }
      } catch {
        // Ignore if not in Tauri context
      }
    };
    setupListener();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [autoPause, autoResume]);

  // ── Cleanup on unmount ──

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
      windowBindingRef.current.dispose();
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
