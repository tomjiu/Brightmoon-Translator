import { invoke } from "@tauri-apps/api/core";
import { createWorker, Worker } from "tesseract.js";

let worker: Worker | null = null;

export interface ScreenshotSnapshotInfo {
  screenX: number;
  screenY: number;
  screenWidth: number;
  screenHeight: number;
  scaleFactor: number;
  imageWidth: number;
  imageHeight: number;
}

export interface ScreenshotSnapshot {
  image: string;
  info: ScreenshotSnapshotInfo;
}

export interface ScreenshotRegion {
  left: number;
  top: number;
  width: number;
  height: number;
}

async function getWorker(): Promise<Worker> {
  if (!worker) {
    worker = await createWorker("chi_sim+eng", 1, {
      logger: (m) => {
        if (m.status === "recognizing text") {
          console.log(`OCR progress: ${Math.round(m.progress * 100)}%`);
        }
      },
    });
  }
  return worker;
}

export async function captureScreen(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<string> {
  return await invoke<string>("capture_screen", {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
  });
}

export async function captureFullScreen(): Promise<string> {
  return await invoke<string>("capture_full_screen");
}

export async function prepareScreenshotSnapshot(): Promise<ScreenshotSnapshotInfo> {
  return await invoke<ScreenshotSnapshotInfo>("prepare_screenshot_snapshot");
}

export async function loadScreenshotSnapshot(): Promise<ScreenshotSnapshot> {
  return await invoke<ScreenshotSnapshot>("load_screenshot_snapshot");
}

export async function cropScreenshotSnapshot(region: ScreenshotRegion): Promise<string> {
  return await invoke<string>("crop_screenshot_snapshot", {
    left: Math.round(region.left),
    top: Math.round(region.top),
    width: Math.round(region.width),
    height: Math.round(region.height),
  });
}

export async function captureScreenshotRegion(region: ScreenshotRegion): Promise<string> {
  return await invoke<string>("capture_screenshot_region", {
    left: Math.round(region.left),
    top: Math.round(region.top),
    width: Math.round(region.width),
    height: Math.round(region.height),
  });
}

export async function systemOcrImage(imageDataUrl: string, lang = "auto"): Promise<string> {
  return await invoke<string>("system_ocr", {
    base64Data: imageDataUrl,
    lang,
  });
}

export async function ocrImage(imageDataUrl: string): Promise<string> {
  const w = await getWorker();
  const {
    data: { text },
  } = await w.recognize(imageDataUrl);
  return text.trim();
}

export async function ocrImagePreferNative(imageDataUrl: string, lang = "auto"): Promise<string> {
  try {
    const text = await systemOcrImage(imageDataUrl, lang);
    if (text.trim()) {
      console.log("[OCR] Engine: Windows.Media.Ocr (WinRT)");
      return text.trim();
    }
    console.warn("[OCR] WinRT returned empty, falling back to tesseract.js");
  } catch (err) {
    console.warn("[OCR] WinRT failed, falling back to tesseract.js:", err);
  }
  const text = await ocrImage(imageDataUrl);
  console.log("[OCR] Engine: tesseract.js");
  return text;
}

export async function ocrScreenRegion(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<{ image: string; text: string }> {
  const image = await captureScreen(x, y, width, height);
  const text = await ocrImage(image);
  return { image, text };
}

export async function createOverlay(
  x: number,
  y: number,
  width: number,
  height: number,
  text: string
): Promise<void> {
  await invoke("create_overlay", {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
    text,
  });
}

export async function closeOverlay(): Promise<void> {
  await invoke("close_overlay");
}

export async function terminateOcrWorker(): Promise<void> {
  if (worker) {
    await worker.terminate();
    worker = null;
  }
}
