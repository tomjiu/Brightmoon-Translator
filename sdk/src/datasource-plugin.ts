/**
 * Moon Translator Plugin SDK - DataSource Plugin Base
 *
 * Base class for data source plugins (TM, glossary, dictionary).
 */

import { BasePlugin } from "./base-plugin.js";
import type { Match } from "./types.js";

/**
 * Base class for data source plugins.
 *
 * Subclasses must implement at least `lookup()`.
 *
 * @example
 * ```ts
 * class MyDatasource extends DatasourcePlugin {
 *   protected async lookup(query: string, from: string, to: string): Promise<Match[]> {
 *     const results = await searchMyDatabase(query, from, to);
 *     return results.map(r => ({ source: r.src, target: r.tgt, similarity: r.score }));
 *   }
 * }
 *
 * new MyDatasource().run();
 * ```
 */
export abstract class DatasourcePlugin extends BasePlugin {
  /**
   * Implement the lookup/search logic.
   * Returns matching translation pairs from the data source.
   */
  protected abstract lookup(
    query: string,
    from: string,
    to: string,
    threshold?: number,
    limit?: number,
  ): Promise<Match[]>;

  /**
   * Optional: add an entry to the data source.
   */
  protected async addEntry(
    _source: string,
    _target: string,
    _from: string,
    _to: string,
  ): Promise<void> {
    throw new Error("This data source does not support adding entries");
  }

  protected override async onInit(): Promise<void> {
    this.logger.info("DataSource plugin ready");
  }

  /**
   * Perform a lookup. Called by the host via HTTP endpoint.
   */
  async search(
    query: string,
    from: string,
    to: string,
    threshold?: number,
    limit?: number,
  ): Promise<Match[]> {
    const finish = this.tracer.startRequest(
      this.pluginName,
      "POST",
      "lookup",
      `query=${query}, ${from}→${to}`,
    );

    try {
      const result = await this.perf.measure("lookup", () =>
        this.lookup(query, from, to, threshold, limit),
      );
      finish(200, `matches=${result.length}`);
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      finish(500, undefined, msg);
      throw err;
    }
  }
}
