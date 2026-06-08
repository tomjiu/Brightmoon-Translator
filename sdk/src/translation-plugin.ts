/**
 * Moon Translator Plugin SDK - Translation Plugin Base
 *
 * Base class for translation plugins running as sandboxed subprocesses.
 * Subclasses implement the actual translation logic.
 */

import { BasePlugin } from "./base-plugin.js";
import type { PluginToHostMessage } from "./types.js";

/**
 * Base class for translation plugins.
 *
 * Subclasses must implement `translate()`.
 *
 * @example
 * ```ts
 * class MyTranslator extends TranslationPlugin {
 *   protected async translate(text: string, from: string, to: string): Promise<string> {
 *     const resp = await fetch("https://api.example.com/translate", {
 *       method: "POST",
 *       headers: { "Content-Type": "application/json" },
 *       body: JSON.stringify({ text, source: from, target: to }),
 *     });
 *     const data = await resp.json();
 *     return data.translated;
 *   }
 * }
 *
 * new MyTranslator().run();
 * ```
 */
export abstract class TranslationPlugin extends BasePlugin {
  /**
   * Implement the translation logic.
   *
   * @param text - Source text to translate
   * @param from - Source language code ("auto" for auto-detect)
   * @param to - Target language code
   * @returns Translated text
   */
  protected abstract translate(text: string, from: string, to: string): Promise<string>;

  /**
   * Optional: handle batch translation requests.
   * Default implementation translates each text sequentially.
   */
  protected async translateBatch(texts: string[], from: string, to: string): Promise<string[]> {
    const results: string[] = [];
    for (const text of texts) {
      results.push(await this.translate(text, from, to));
    }
    return results;
  }

  protected override async onInit(): Promise<void> {
    this.requirePermission("network");
    this.logger.info("Translation plugin ready");
  }

  protected override async onTranslate(
    requestId: string,
    text: string,
    from: string,
    to: string,
  ): Promise<string> {
    const finish = this.tracer.startRequest(
      this.pluginName,
      "POST",
      "translate",
      `text=${text.length}chars, ${from}→${to}`,
    );

    try {
      const result = await this.perf.measure("translate", () =>
        this.translate(text, from, to),
      );
      finish(200, `translated=${result.length}chars`);
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      finish(500, undefined, msg);
      throw err;
    }
  }
}
