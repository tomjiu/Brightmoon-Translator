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
  // Run Youdao and WinRT in parallel
  const youdaoPromise = youdaoOcrDetailed(imageDataUrl, lang).catch((err) => {
    console.warn("[OCR] Youdao OCR failed:", err);
    return null;
  });

  const winrtPromise = systemOcrImage(imageDataUrl, lang).catch((err) => {
    console.warn("[OCR] WinRT failed:", err);
    return null;
  });

  const [youdaoResult, winrtText] = await Promise.all([youdaoPromise, winrtPromise]);

  // Prefer WinRT first (local, fast, reliable)
  if (winrtText?.trim()) {
    console.log("[OCR] Engine: Windows.Media.Ocr (WinRT)");
    return winrtText.trim();
  }

  // Fallback to Youdao if WinRT fails or returns empty
  if (youdaoResult?.text?.trim()) {
    console.log("[OCR] Engine: Youdao OCR (fallback)");
    return youdaoResult.text.trim();
  }

  console.warn("[OCR] Both Youdao and WinRT returned empty or failed");

  // 3. Fallback: tesseract.js
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
  return await invoke<OcrResultDetailed>("system_ocr_detailed", {
    base64Data: imageDataUrl,
    lang,
  });
}

/** Run Youdao OCR and return detailed per-line results with bounding boxes. */
export async function youdaoOcrDetailed(
  imageDataUrl: string,
  lang = "auto",
): Promise<OcrResultDetailed> {
  return await invoke<OcrResultDetailed>("youdao_ocr", {
    base64Data: imageDataUrl,
    lang,
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
    console.log("[OCR] Engine: Windows.Media.Ocr (WinRT) detailed");
    return winrtResult;
  }

  // Fallback to Youdao if WinRT fails or returns empty
  if (youdaoResult?.text?.trim()) {
    console.log("[OCR] Engine: Youdao OCR (fallback)");
    return youdaoResult;
  }

  console.warn("[OCR] Both Youdao and WinRT returned empty or failed");

  // 3. Fallback: tesseract.js flat text, no bounding boxes
  const text = await ocrImage(imageDataUrl);
  console.log("[OCR] Engine: tesseract.js");
  const lines: OcrLineResult[] = text
    ? [{ text, x: 0, y: 0, width: 0, height: 0, words: [] }]
    : [];
  return { text, lines };
}
