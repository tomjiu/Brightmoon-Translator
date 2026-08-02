import { invokeOrThrow } from './invoke';
import { useConfigStore } from '../stores/configStore';

/** Minimal interface for the tesseract.js worker — only the methods we use. */
interface TesseractBbox {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

interface TesseractWord {
  text: string;
  bbox: TesseractBbox;
}

interface TesseractLine {
  text: string;
  bbox: TesseractBbox;
  words?: TesseractWord[];
}

interface TesseractPage {
  text: string;
  lines?: TesseractLine[];
  words?: TesseractWord[];
}

interface TesseractWorker {
  recognize(image: string): Promise<{ data: TesseractPage }>;
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
  const result = await ocrImageTesseractDetailed(imageDataUrl);
  return result.text;
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

function bboxToRect(b: TesseractBbox): { x: number; y: number; width: number; height: number } {
  const x = b.x0;
  const y = b.y0;
  const width = Math.max(1, b.x1 - b.x0);
  const height = Math.max(1, b.y1 - b.y0);
  return { x, y, width, height };
}

/** Tesseract with real line/word boxes (not zero-size placeholders). */
export async function ocrImageTesseractDetailed(imageDataUrl: string): Promise<OcrResultDetailed> {
  const w = await getWorker();
  const { data } = await w.recognize(imageDataUrl);
  const text = (data.text || '').trim();
  const rawLines = data.lines?.length
    ? data.lines
    : data.words?.length
      ? [
          {
            text,
            bbox: data.words.reduce(
              (acc, word) => ({
                x0: Math.min(acc.x0, word.bbox.x0),
                y0: Math.min(acc.y0, word.bbox.y0),
                x1: Math.max(acc.x1, word.bbox.x1),
                y1: Math.max(acc.y1, word.bbox.y1),
              }),
              {
                x0: data.words[0].bbox.x0,
                y0: data.words[0].bbox.y0,
                x1: data.words[0].bbox.x1,
                y1: data.words[0].bbox.y1,
              },
            ),
            words: data.words,
          } as TesseractLine,
        ]
      : [];

  const lines: OcrLineResult[] = rawLines
    .map((line) => {
      const rect = bboxToRect(line.bbox);
      const words: OcrWordResult[] = (line.words || []).map((word) => {
        const wr = bboxToRect(word.bbox);
        return { text: word.text, ...wr };
      });
      return {
        text: (line.text || '').trim(),
        ...rect,
        words,
      };
    })
    .filter((line) => line.text.length > 0);

  if (lines.length === 0 && text) {
    return {
      text,
      lines: [{ text, x: 0, y: 0, width: 1, height: 1, words: [] }],
    };
  }
  return { text: text || lines.map((l) => l.text).join('\n'), lines };
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

/** Rapid/Paddle offline sidecar via Rust command. */
export async function offlineOcrDetailed(
  imageDataUrl: string,
  backend: 'rapid' | 'paddle',
  pluginDir?: string,
  lang = 'auto',
): Promise<OcrResultDetailed> {
  const cfg = useConfigStore.getState().config;
  return await invokeOrThrow<OcrResultDetailed>('offline_ocr', {
    base64Data: imageDataUrl,
    backend,
    pluginDir: pluginDir ?? cfg.offlineOcr?.pluginDir ?? '',
    lang,
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
    return await ocrImageTesseractDetailed(imageDataUrl);
  } catch {
    return { text: '', lines: [] };
  }
}

/** Layout-aware OCR: runs DocLayout-YOLO region detection first (when enabled
 *  in config), filtering figure/table/formula regions, then OCRs each text
 *  region separately and merges results. Falls back to full-image OCR when
 *  layout detection is disabled, the model is missing, or the feature is not
 *  compiled.
 *
 *  `ocrBackend`: "winrt" or "offline". The backend's `ocr_with_layout_detection`
 *  checks `layout_detection_enabled` config internally, so this is safe to call
 *  unconditionally — when disabled it just delegates to the raw OCR path.
 */
export async function ocrImageWithLayout(
  imageDataUrl: string,
  lang = 'auto',
  ocrBackend: 'winrt' | 'offline' = 'winrt',
): Promise<OcrResultDetailed> {
  return await invokeOrThrow<OcrResultDetailed>('ocr_image_with_layout', {
    base64Data: imageDataUrl,
    lang,
    ocrBackend,
  });
}

/** OCR with configurable engine preference.
 *  engine: "auto" | "winrt" | "youdao" | "tesseract" | "rapid" | "paddle"
 *
 *  When `layoutDetectionEnabled` is true in config, winrt/rapid/paddle/auto
 *  engines route through the layout-detection pipeline (DocLayout-YOLO region
 *  filtering → per-region OCR → merged results). Youdao (cloud service with
 *  own region detection) and tesseract (in-browser) always use their direct
 *  paths.
 */
export async function ocrWithEngine(
  imageDataUrl: string,
  engine = 'auto',
  lang = 'auto',
): Promise<OcrResultDetailed> {
  const cfg = useConfigStore.getState().config;
  const useLayout = cfg.layoutDetectionEnabled === true;

  if (useLayout) {
    switch (engine) {
      case 'winrt':
        return await ocrImageWithLayout(imageDataUrl, lang, 'winrt');

      case 'rapid':
      case 'paddle':
        return await ocrImageWithLayout(imageDataUrl, lang, 'offline');

      case 'auto':
      default:
        // Layout pipeline with winrt backend; the region filtering improves
        // accuracy enough that the youdao/tesseract fallback chain is rarely
        // needed. If winrt fails entirely, ocrImageWithLayout returns an error
        // which the caller can handle.
        return await ocrImageWithLayout(imageDataUrl, lang, 'winrt');
    }
  }

  switch (engine) {
    case 'winrt':
      return await ocrImageDetailed(imageDataUrl, lang);

    case 'youdao':
      return await youdaoOcrDetailed(imageDataUrl, lang);

    case 'tesseract':
      return await ocrImageTesseractDetailed(imageDataUrl);

    case 'rapid':
    case 'paddle':
      return await offlineOcrDetailed(imageDataUrl, engine, undefined, lang);

    case 'auto':
    default:
      return await ocrImagePreferNativeDetailed(imageDataUrl, lang);
  }
}
