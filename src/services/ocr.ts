import { invokeOrThrow } from "./invoke";
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
    worker = await createWorker("chi_sim+eng", 1);
  }
  return worker;
}

export async function captureScreen(
  x: number,
  y: number,
  width: number,
  height: number
): Promise<string> {
  return await invokeOrThrow<string>("capture_screen", {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
  });
}

export async function prepareScreenshotSnapshot(): Promise<ScreenshotSnapshotInfo> {
  return await invokeOrThrow<ScreenshotSnapshotInfo>("prepare_screenshot_snapshot");
}

export async function loadScreenshotSnapshot(): Promise<ScreenshotSnapshot> {
  return await invokeOrThrow<ScreenshotSnapshot>("load_screenshot_snapshot");
}

export async function cropScreenshotSnapshot(region: ScreenshotRegion): Promise<string> {
  return await invokeOrThrow<string>("crop_screenshot_snapshot", {
    left: Math.round(region.left),
    top: Math.round(region.top),
    width: Math.round(region.width),
    height: Math.round(region.height),
  });
}

export async function captureScreenshotRegion(region: ScreenshotRegion): Promise<string> {
  return await invokeOrThrow<string>("capture_screenshot_region", {
    left: Math.round(region.left),
    top: Math.round(region.top),
    width: Math.round(region.width),
    height: Math.round(region.height),
  });
}

export async function ocrImage(imageDataUrl: string): Promise<string> {
  const w = await getWorker();
  const {
    data: { text },
  } = await w.recognize(imageDataUrl);
  return text.trim();
}

export async function createOverlay(
  x: number,
  y: number,
  width: number,
  height: number,
  text: string
): Promise<void> {
  await invokeOrThrow("create_overlay", {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
    text,
  });
}

export async function closeOverlay(): Promise<void> {
  await invokeOrThrow("close_overlay");
}

// ── Detailed OCR with per-line bounding boxes ──────────────────────────────

export interface OcrWordResult {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface OcrLineResult {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  words: OcrWordResult[];
}

export interface OcrResultDetailed {
  lines: OcrLineResult[];
  text: string;
}

/** Run WinRT OCR and return detailed per-line results with bounding boxes. */
export async function ocrImageDetailed(
  imageDataUrl: string,
  lang = "auto",
): Promise<OcrResultDetailed> {
  return await invokeOrThrow<OcrResultDetailed>("system_ocr_detailed", {
    base64Data: imageDataUrl,
    lang,
  });
}

/** Run Youdao OCR and return detailed per-line results with bounding boxes. */
export async function youdaoOcrDetailed(
  imageDataUrl: string,
  lang = "auto",
  appKey?: string,
  appSecret?: string,
): Promise<OcrResultDetailed> {
  return await invokeOrThrow<OcrResultDetailed>("youdao_ocr", {
    base64Data: imageDataUrl,
    lang,
    appKey,
    appSecret,
  });
}

/** Prefer WinRT OCR, fall back to Youdao, then tesseract.js.
 *  Runs WinRT and Youdao in parallel for faster results. */
export async function ocrImagePreferNativeDetailed(
  imageDataUrl: string,
  lang = "auto",
): Promise<OcrResultDetailed> {
  // Run Youdao and WinRT in parallel
  const youdaoPromise = youdaoOcrDetailed(imageDataUrl, lang).catch((err) => {
    console.warn("[OCR] Youdao OCR failed:", err);
    return null;
  });

  const winrtPromise = ocrImageDetailed(imageDataUrl, lang).catch((err) => {
    console.warn("[OCR] WinRT detailed failed:", err);
    return null;
  });

  // Wait for both to complete
  const [youdaoResult, winrtResult] = await Promise.all([youdaoPromise, winrtPromise]);

  // Prefer WinRT first (local, fast, reliable)
  if (winrtResult?.text?.trim()) {
    return winrtResult;
  }

  // Fallback to Youdao if WinRT fails or returns empty
  if (youdaoResult?.text?.trim()) {
    return youdaoResult;
  }

  console.warn("[OCR] Both Youdao and WinRT returned empty or failed");

  // 3. Fallback: tesseract.js flat text, no bounding boxes
  const text = await ocrImage(imageDataUrl);
  const lines: OcrLineResult[] = text
    ? [{ text, x: 0, y: 0, width: 0, height: 0, words: [] }]
    : [];
  return { text, lines };
}
