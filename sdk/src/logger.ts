/**
 * Moon Translator Plugin SDK - Logger
 *
 * Structured logging for plugin processes. Logs are sent to stderr so they
 * do not interfere with the IPC stdout protocol.
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

const LOG_LEVEL_PRIORITY: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

/**
 * Plugin logger that writes structured JSON to stderr.
 *
 * The host captures stderr output and can display it in the plugin debug UI.
 */
export class PluginLogger {
  private minLevel: LogLevel;
  private pluginName: string;

  constructor(pluginName: string, minLevel: LogLevel = "info") {
    this.pluginName = pluginName;
    this.minLevel = minLevel;
  }

  debug(message: string, data?: Record<string, unknown>): void {
    this.log("debug", message, data);
  }

  info(message: string, data?: Record<string, unknown>): void {
    this.log("info", message, data);
  }

  warn(message: string, data?: Record<string, unknown>): void {
    this.log("warn", message, data);
  }

  error(message: string, data?: Record<string, unknown>): void {
    this.log("error", message, data);
  }

  /** Create a child logger with a suffix appended to the plugin name */
  child(suffix: string): PluginLogger {
    return new PluginLogger(`${this.pluginName}:${suffix}`, this.minLevel);
  }

  setLevel(level: LogLevel): void {
    this.minLevel = level;
  }

  private log(level: LogLevel, message: string, data?: Record<string, unknown>): void {
    if (LOG_LEVEL_PRIORITY[level] < LOG_LEVEL_PRIORITY[this.minLevel]) return;

    const entry = {
      ts: new Date().toISOString(),
      level,
      plugin: this.pluginName,
      msg: message,
      ...(data && Object.keys(data).length > 0 ? { data } : {}),
    };

    process.stderr.write(JSON.stringify(entry) + "\n");
  }
}
