import { useState, useRef, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { captureScreen, ocrImage } from "../services/ocr";
import { useTranslateStore } from "../stores/translateStore";

interface OcrRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface OcrMonitorState {
  isMonitoring: boolean;
  region: OcrRegion | null;
  lastText: string;
  interval: number; // ms
  clickThrough: boolean;
  pinned: boolean;
}

export function useOcrMonitor() {
  const [state, setState] = useState<OcrMonitorState>({
    isMonitoring: false,
    region: null,
    lastText: "",
    interval: 2000,
    clickThrough: false,
    pinned: false,
  });

  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const lastTextRef = useRef<string>("");
  const { setSourceText, translate } = useTranslateStore();

  // Stop monitoring
  const stopMonitoring = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    setState((prev) => ({ ...prev, isMonitoring: false }));
  }, []);

  // Capture and OCR a region
  const captureAndOcr = useCallback(
    async (region: OcrRegion) => {
      try {
        // Capture the region
        const image = await captureScreen(
          region.x,
          region.y,
          region.width,
          region.height
        );

        // OCR the image
        const text = await ocrImage(image);

        // Check if text changed
        if (text && text !== lastTextRef.current) {
          lastTextRef.current = text;
          setState((prev) => ({ ...prev, lastText: text }));

          // Auto translate
          setSourceText(text);
          await translate();

          // Show overlay
          const result = useTranslateStore.getState().results[0];
          if (result) {
            await invoke("create_overlay", {
              x: region.x + region.width + 10,
              y: region.y,
              width: 350,
              height: 200,
              text: result.text,
              showControls: true,
            });
          }
        }
      } catch (e) {
        console.error("OCR monitor error:", e);
      }
    },
    [setSourceText, translate]
  );

  // Start monitoring a region
  const startMonitoring = useCallback(
    (region: OcrRegion, interval: number = 2000) => {
      stopMonitoring();

      setState({
        isMonitoring: true,
        region,
        lastText: "",
        interval,
        clickThrough: false,
        pinned: false,
      });

      lastTextRef.current = "";

      // Initial capture
      captureAndOcr(region);

      // Set up interval
      timerRef.current = setInterval(() => {
        captureAndOcr(region);
      }, interval);
    },
    [captureAndOcr, stopMonitoring]
  );

  // Toggle click-through
  const toggleClickThrough = useCallback(async () => {
    const newValue = !state.clickThrough;
    await invoke("set_overlay_click_through", { ignore: newValue });
    setState((prev) => ({ ...prev, clickThrough: newValue }));
  }, [state.clickThrough]);

  // Toggle pin
  const togglePin = useCallback(async () => {
    const newValue = !state.pinned;
    if (newValue) {
      await invoke("pin_overlay");
    }
    setState((prev) => ({ ...prev, pinned: newValue }));
  }, [state.pinned]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) {
        clearInterval(timerRef.current);
      }
    };
  }, []);

  return {
    ...state,
    startMonitoring,
    stopMonitoring,
    toggleClickThrough,
    togglePin,
  };
}
