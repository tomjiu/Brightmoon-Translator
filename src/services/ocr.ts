import { invokeOrThrow } from './invoke';

/** Minimal interface for the tesseract.js worker — only the methods we use. */
interface TesseractWorker {
  recognize(image: string): Promise<{ data: { text: string } }>;
}

let worker: TesseractWorker | null = null;

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
  /** Absolute path; use convertFileSrc (pot-desktop). Not a data URL. */
  imagePath: string;
  info: ScreenshotSnapshotInfo;
}

export interface ScreenshotRegion {
  left: number;
  top: number;
  width: number;
  height: number;
}

async function getWorker(): Promise<TesseractWorker> {
  if (!worker) {
    const { createWorker } = await import('tesseract.js');
    worker = await createWorker('chi_sim+eng', 1);
  }
  return worker;
}

export async function captureScreen(
  x: number,
  y: number,
  width: number,
  height: number,
): Promise<string> {
  return await invokeOrThrow<string>('capture_screen', {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
  });
}

/** @param forceRefresh skip 30s smart cache — always capture a new full-screen snapshot */
export async function prepareScreenshotSnapshot(
  forceRefresh = true,
): Promise<ScreenshotSnapshotInfo> {
  return await invokeOrThrow<ScreenshotSnapshotInfo>('prepare_screenshot_snapshot', {
    forceRefresh,
  });
}

export async function loadScreenshotSnapshot(): Promise<ScreenshotSnapshot> {
  return await invokeOrThrow<ScreenshotSnapshot>('load_screenshot_snapshot');
}

export async function captureScreenshotRegion(region: ScreenshotRegion): Promise<string> {
  return await invokeOrThrow<string>('capture_screenshot_region', {
    left: Math.round(region.left),
    top: Math.round(region.top),
    width: Math.round(region.width),
    height: Math.round(region.height),
  });
}

/** Crop the cached full-screen snapshot (image-pixel coords). Prefer this for the
 *  initial selection so the crop matches exactly what the user saw on the selector. */
export async function cropScreenshotSnapshot(region: ScreenshotRegion): Promise<string> {
  return await invokeOrThrow<string>('crop_screenshot_snapshot', {
    left: Math.max(0, Math.round(region.left)),
    top: Math.max(0, Math.round(region.top)),
    width: Math.max(1, Math.round(region.width)),
    height: Math.max(1, Math.round(region.height)),
  });
}

/** Pixel-grid fingerprint (Rust). Falls back to empty on failure — caller uses JS hash. */
export async function imageDataUrlFingerprint(dataUrl: string): Promise<string> {
  if (!dataUrl) return '';
  try {
    return await invokeOrThrow<string>('image_data_url_fingerprint', { dataUrl });
  } catch {
    return '';
  }
}

export async function ocrImage(imageDataUrl: string): Promise<string> {
  const w = await getWorker();
  const {
    data: { text },
  } = await w.recognize(imageDataUrl);
  return text.trim();
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
  lang = 'auto',
): Promise<OcrResultDetailed> {
  return await invokeOrThrow<OcrResultDetailed>('system_ocr_detailed', {
    base64Data: imageDataUrl,
    lang,
  });
}

/** Run Youdao OCR and return detailed per-line results with bounding boxes. */
export async function youdaoOcrDetailed(
  imageDataUrl: string,
  lang = 'auto',
  appKey?: string,
  appSecret?: string,
): Promise<OcrResultDetailed> {
  return await invokeOrThrow<OcrResultDetailed>('youdao_ocr', {
    base64Data: imageDataUrl,
    lang,
    appKey,
    appSecret,
  });
}

/** Prefer WinRT (accurate boxes), then Youdao, then tesseract — sequential (not parallel).
 *  Parallel double-billed every refresh/watch tick and inflated latency. */
export async function ocrImagePreferNativeDetailed(
  imageDataUrl: string,
  lang = 'auto',
): Promise<OcrResultDetailed> {
  try {
    const winrt = await ocrImageDetailed(imageDataUrl, lang);
    if (winrt.text.trim()) return winrt;
  } catch (err: unknown) {
    console.warn('[OCR] WinRT detailed failed:', err);
  }

  try {
    const youdao = await youdaoOcrDetailed(imageDataUrl, lang);
    if (youdao.text.trim()) return youdao;
  } catch (err: unknown) {
    console.warn('[OCR] Youdao OCR failed:', err);
  }

  try {
    const text = await ocrImage(imageDataUrl);
    const lines: OcrLineResult[] = text
      ? [{ text, x: 0, y: 0, width: 0, height: 0, words: [] }]
      : [];
    return { text: text || '', lines };
  } catch {
    return { text: '', lines: [] };
  }
}

/** OCR with configurable engine preference.
 *  engine: "auto" | "winrt" | "youdao" | "tesseract"
 */
export async function ocrWithEngine(
  imageDataUrl: string,
  engine = 'auto',
  lang = 'auto',
): Promise<OcrResultDetailed> {
  switch (engine) {
    case 'winrt':
      return await ocrImageDetailed(imageDataUrl, lang);

    case 'youdao':
      return await youdaoOcrDetailed(imageDataUrl, lang);

    case 'tesseract': {
      const text = await ocrImage(imageDataUrl);
      const lines: OcrLineResult[] = text
        ? [{ text, x: 0, y: 0, width: 0, height: 0, words: [] }]
        : [];
      return { text, lines };
    }

    case 'auto':
    default:
      return await ocrImagePreferNativeDetailed(imageDataUrl, lang);
  }
}
