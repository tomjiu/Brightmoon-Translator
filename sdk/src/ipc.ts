/**
 * Moon Translator Plugin SDK - IPC Communication
 *
 * Handles stdin/stdout JSON-line IPC between a sandboxed plugin subprocess
 * and the host application.
 */

import type { HostToPluginMessage, PluginToHostMessage } from "./types.js";

type MessageHandler = (msg: HostToPluginMessage) => void;

/**
 * Stdin/stdout JSON-line IPC channel for sandboxed plugin processes.
 *
 * Plugin processes communicate with the host by writing newline-delimited
 * JSON to stdout and reading from stdin.
 */
export class PluginIpc {
  private handlers: Map<string, MessageHandler[]> = new Map();
  private buffer = "";
  private closed = false;

  constructor() {
    this.setupStdin();
  }

  /** Register a handler for a specific message type */
  on(type: string, handler: MessageHandler): void {
    const existing = this.handlers.get(type) ?? [];
    existing.push(handler);
    this.handlers.set(type, existing);
  }

  /** Remove a handler */
  off(type: string, handler: MessageHandler): void {
    const existing = this.handlers.get(type);
    if (existing) {
      const idx = existing.indexOf(handler);
      if (idx >= 0) existing.splice(idx, 1);
    }
  }

  /** Send a message to the host (writes to stdout) */
  send(msg: PluginToHostMessage): void {
    if (this.closed) return;
    const json = JSON.stringify(msg);
    process.stdout.write(json + "\n");
  }

  /** Close the IPC channel */
  close(): void {
    this.closed = true;
    this.handlers.clear();
  }

  private setupStdin(): void {
    process.stdin.setEncoding("utf-8");
    process.stdin.on("data", (chunk: string) => {
      this.buffer += chunk;
      this.processBuffer();
    });
    process.stdin.on("end", () => {
      this.closed = true;
    });
  }

  private processBuffer(): void {
    const lines = this.buffer.split("\n");
    // Keep incomplete last line in buffer
    this.buffer = lines.pop() ?? "";

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;

      try {
        const msg = JSON.parse(trimmed) as HostToPluginMessage;
        this.dispatch(msg);
      } catch {
        // Ignore malformed lines
      }
    }
  }

  private dispatch(msg: HostToPluginMessage): void {
    const handlers = this.handlers.get(msg.type) ?? [];
    for (const handler of handlers) {
      try {
        handler(msg);
      } catch {
        // Handler errors are non-fatal
      }
    }

    // Also dispatch to wildcard listeners
    const wildcardHandlers = this.handlers.get("*") ?? [];
    for (const handler of wildcardHandlers) {
      try {
        handler(msg);
      } catch {
        // Non-fatal
      }
    }
  }
}
