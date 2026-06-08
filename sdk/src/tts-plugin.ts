/**
 * Moon Translator Plugin SDK - TTS Plugin Base
 *
 * Base class for TTS plugins running as sandboxed subprocesses.
 */

import { BasePlugin } from "./base-plugin.js";
import type { TtsVoice } from "./types.js";

/** TTS result returned by the plugin */
export interface TtsResult {
  audio: string;
  format?: string;
  durationMs?: number;
}

/**
 * Base class for TTS plugins.
 *
 * Subclasses must implement `synthesize()`.
 *
 * @example
 * ```ts
 * class MyTts extends TtsPlugin {
 *   protected async synthesize(text: string, lang?: string, voice?: string): Promise<TtsResult> {
 *     const audio = await callExternalTts(text, lang, voice);
 *     return { audio: base64Encode(audio), format: "mp3" };
 *   }
 * }
 *
 * new MyTts().run();
 * ```
 */
export abstract class TtsPlugin extends BasePlugin {
  /**
   * Implement the TTS synthesis logic.
   *
   * @param text - Text to convert to speech
   * @param lang - Optional language code
   * @param voice - Optional voice ID
   * @returns TTS result with base64-encoded audio
   */
  protected abstract synthesize(text: string, lang?: string, voice?: string): Promise<TtsResult>;

  /**
   * Optional: return available voices. Default returns an empty list.
   */
  protected async getVoices(): Promise<TtsVoice[]> {
    return [];
  }

  protected override async onInit(): Promise<void> {
    this.requirePermission("tts");
    this.logger.info("TTS plugin ready");
  }

  /**
   * Synthesize text to audio. Called by the host via HTTP endpoint.
   */
  async synthesizeText(text: string, lang?: string, voice?: string): Promise<TtsResult> {
    const finish = this.tracer.startRequest(
      this.pluginName,
      "POST",
      "tts",
      `text=${text.length}chars, lang=${lang ?? "auto"}, voice=${voice ?? "default"}`,
    );

    try {
      const result = await this.perf.measure("tts", () =>
        this.synthesize(text, lang, voice),
      );
      finish(200, `audio=${result.audio.length}chars, format=${result.format ?? "mp3"}`);
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      finish(500, undefined, msg);
      throw err;
    }
  }
}
