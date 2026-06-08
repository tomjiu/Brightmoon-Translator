/**
 * Moon Translator Plugin SDK - OCR Plugin Base
 *
 * Base class for OCR plugins running as sandboxed subprocesses.
 */

import { BasePlugin } from "./base-plugin.js";
import type { OcrLine } from "./types.js";

/** OCR result returned by the plugin */
export interface OcrResult {
  text: string;
  lines?: OcrLine[];
}

/**
 * Base class for OCR plugins.
 *
 * Subclasses must implement `recognize()`.
 *
 * @example
 * ```ts
 * class MyOcr extends OcrPlugin {
 *   protected async recognize(image: string, lang?: string): Promise<OcrResult> {
 *     const result = await callExternalOcr(image, lang);
 *     return { text: result.text, lines: result.lines };
 *   }
 * }
 *
 * new MyOcr().run();
 * ```
 */
export abstract class OcrPlugin extends BasePlugin {
  /**
   * Implement the OCR recognition logic.
   *
   * @param image - Base64-encoded image data
   * @param lang - Optional language hint
   * @param detailed - Whether to return detailed results with bounding boxes
   * @returns OCR result
   */
  protected abstract recognize(image: string, lang?: string, detailed?: boolean): Promise<OcrResult>;

  protected override async onInit(): Promise<void> {
    this.requirePermission("ocr");
    this.logger.info("OCR plugin ready");
  }

  /**
   * Handle an OCR request. Called by the host via a custom message.
   * Since the sandboxed IPC protocol only has Translate for text,
   * OCR plugins should expose an HTTP endpoint and be called directly.
   */
  async recognizeImage(image: string, lang?: string, detailed?: boolean): Promise<OcrResult> {
    const finish = this.tracer.startRequest(
      this.pluginName,
      "POST",
      "ocr",
      `image=${image.length}chars, lang=${lang ?? "auto"}`,
    );

    try {
      const result = await this.perf.measure("ocr", () =>
        this.recognize(image, lang, detailed),
      );
      finish(200, `text=${result.text.length}chars, lines=${result.lines?.length ?? 0}`);
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      finish(500, undefined, msg);
      throw err;
    }
  }
}
