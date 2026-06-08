/**
 * Moon Translator Plugin SDK - Debug & Tracing Tools
 *
 * Provides API call tracing, performance measurement, and health monitoring
 * utilities for plugin developers.
 */

import type { ApiCallRecord } from "./types.js";

// ---------------------------------------------------------------------------
// API Call Tracer
// ---------------------------------------------------------------------------

/**
 * Records API calls made by a plugin for debugging and inspection.
 * Keeps a bounded ring buffer of recent calls.
 */
export class ApiCallTracer {
  private records: ApiCallRecord[] = [];
  private maxRecords: number;
  private counter = 0;

  constructor(maxRecords = 200) {
    this.maxRecords = maxRecords;
  }

  /**
   * Record the start of an outgoing request. Returns a finish function
   * that should be called when the response arrives.
   */
  startRequest(
    pluginName: string,
    method: string,
    url: string,
    requestSummary?: string,
  ): (status: number, responseSummary?: string, error?: string) => void {
    const id = `call_${++this.counter}`;
    const startTime = Date.now();

    this.add({
      id,
      pluginName,
      direction: "request",
      method,
      url,
      timestamp: startTime,
      requestSummary,
    });

    return (status: number, responseSummary?: string, error?: string) => {
      const durationMs = Date.now() - startTime;
      this.add({
        id,
        pluginName,
        direction: error ? "error" : "response",
        method,
        url,
        timestamp: Date.now(),
        durationMs,
        status,
        requestSummary,
        responseSummary,
        error,
      });
    };
  }

  /** Get all recorded API calls */
  getRecords(): ApiCallRecord[] {
    return [...this.records];
  }

  /** Get records for a specific plugin */
  getRecordsFor(pluginName: string): ApiCallRecord[] {
    return this.records.filter((r) => r.pluginName === pluginName);
  }

  /** Get the last N records */
  getRecent(count: number): ApiCallRecord[] {
    return this.records.slice(-count);
  }

  /** Clear all records */
  clear(): void {
    this.records = [];
  }

  /** Get total number of recorded calls */
  get count(): number {
    return this.records.length;
  }

  private add(record: ApiCallRecord): void {
    this.records.push(record);
    if (this.records.length > this.maxRecords) {
      this.records = this.records.slice(-this.maxRecords);
    }
  }
}

// ---------------------------------------------------------------------------
// Performance Timer
// ---------------------------------------------------------------------------

/**
 * Simple performance measurement utility.
 */
export class PerfTimer {
  private marks: Map<string, number> = new Map();
  private measures: { name: string; durationMs: number }[] = [];

  /** Start timing a named operation */
  start(name: string): void {
    this.marks.set(name, Date.now());
  }

  /** Stop timing and record the measurement */
  end(name: string): number {
    const start = this.marks.get(name);
    if (start === undefined) return 0;
    const durationMs = Date.now() - start;
    this.marks.delete(name);
    this.measures.push({ name, durationMs });
    return durationMs;
  }

  /** Time an async operation */
  async measure<T>(name: string, fn: () => Promise<T>): Promise<T> {
    this.start(name);
    try {
      const result = await fn();
      this.end(name);
      return result;
    } catch (err) {
      this.end(name);
      throw err;
    }
  }

  /** Get all recorded measurements */
  getMeasures(): { name: string; durationMs: number }[] {
    return [...this.measures];
  }

  /** Get average duration for a named operation */
  getAverage(name: string): number {
    const matching = this.measures.filter((m) => m.name === name);
    if (matching.length === 0) return 0;
    return matching.reduce((sum, m) => sum + m.durationMs, 0) / matching.length;
  }

  /** Clear all measurements */
  clear(): void {
    this.marks.clear();
    this.measures = [];
  }
}

// ---------------------------------------------------------------------------
// Health Monitor
// ---------------------------------------------------------------------------

/** Health check result */
export interface HealthStatus {
  healthy: boolean;
  uptimeMs: number;
  memoryMb: number;
  requestCount: number;
  errorCount: number;
  lastError?: string;
  details?: Record<string, unknown>;
}

/**
 * Tracks plugin health metrics. Plugins can periodically report their status
 * to the host using these metrics.
 */
export class HealthMonitor {
  private startedAt: number;
  private requestCount = 0;
  private errorCount = 0;
  private lastError?: string;

  constructor() {
    this.startedAt = Date.now();
  }

  /** Record a successful request */
  recordRequest(): void {
    this.requestCount++;
  }

  /** Record an error */
  recordError(error: string): void {
    this.errorCount++;
    this.lastError = error;
  }

  /** Get current health status */
  getStatus(): HealthStatus {
    const memUsage = process.memoryUsage();
    return {
      healthy: this.errorCount === 0 || (this.requestCount > 0 && this.errorCount / this.requestCount < 0.5),
      uptimeMs: Date.now() - this.startedAt,
      memoryMb: Math.round((memUsage.heapUsed / 1024 / 1024) * 100) / 100,
      requestCount: this.requestCount,
      errorCount: this.errorCount,
      lastError: this.lastError,
    };
  }

  /** Reset counters */
  reset(): void {
    this.requestCount = 0;
    this.errorCount = 0;
    this.lastError = undefined;
  }
}
