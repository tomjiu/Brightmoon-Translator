import { invoke } from "@tauri-apps/api/core";
import { createWorker, Worker } from "tesseract.js";

let worker: Worker | null = null;

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

export async function ocrImage(imageDataUrl: string): Promise<string> {
  const w = await getWorker();
  const {
    data: { text },
  } = await w.recognize(imageDataUrl);
  return text.trim();
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
