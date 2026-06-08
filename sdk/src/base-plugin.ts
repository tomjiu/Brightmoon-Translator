/**
 * Moon Translator Plugin SDK - Base Plugin
 *
 * Abstract base class for all Moon Translator plugins running as sandboxed
 * subprocesses. Handles IPC initialization, lifecycle management, and
 * provides shared utilities.
 */

import { PluginIpc } from "./ipc.js";
import { PluginLogger } from "./logger.js";
import { ApiCallTracer, PerfTimer, HealthMonitor } from "./debug.js";
import type {
  HostToPluginMessage,
  PluginToHostMessage,
  PluginPermission,
} from "./types.js";

/** Initialization data received from the host */
export interface PluginInitData {
  pluginName: string;
  pluginDir: string;
  permissions: string[];
}

/**
 * Abstract base class for all Moon Translator sandboxed plugins.
 *
 * Lifecycle:
 * 1. Host spawns the plugin process
 * 2. Host sends Init message with plugin metadata
 * 3. Plugin responds with InitOk
 * 4. Plugin handles Translate/Ping messages until Shutdown
 *
 * Subclasses must implement `onInit()` and at least one message handler.
 */
export abstract class BasePlugin {
  /** IPC channel to the host */
  protected ipc: PluginIpc;

  /** Structured logger */
  protected logger: PluginLogger;

  /** API call tracer for debugging */
  protected tracer: ApiCallTracer;

  /** Performance timer */
  protected perf: PerfTimer;

  /** Health monitor */
  protected health: HealthMonitor;

  /** Plugin name received during init */
  protected pluginName = "";

  /** Plugin directory received during init */
  protected pluginDir = "";

  /** Permissions granted by the host */
  protected permissions: PluginPermission[] = [];

  /** Whether the plugin has been initialized */
  protected initialized = false;

  constructor() {
    this.ipc = new PluginIpc();
    this.logger = new PluginLogger("uninitialized");
    this.tracer = new ApiCallTracer();
    this.perf = new PerfTimer();
    this.health = new HealthMonitor();

    this.registerHandlers();
  }

  // -----------------------------------------------------------------------
  // Abstract methods - subclasses must implement
  // -----------------------------------------------------------------------

  /**
   * Called after the plugin receives the Init message from the host.
   * Use this to set up resources (open connections, load config, etc.)
   */
  protected abstract onInit(): Promise<void>;

  /**
   * Called when the host requests a translation.
   * Only relevant for translation-type plugins.
   */
  protected async onTranslate(
    _requestId: string,
    _text: string,
    _from: string,
    _to: string,
  ): Promise<string> {
    throw new Error("This plugin does not support translation");
  }

  /**
   * Called before the plugin shuts down.
   * Use this to clean up resources.
   */
  protected async onShutdown(): Promise<void> {
    // Default: no-op
  }

  // -----------------------------------------------------------------------
  // Public API
  // -----------------------------------------------------------------------

  /**
   * Start the plugin. Call this from the plugin's entry point.
   *
   * @example
   * ```ts
   * const plugin = new MyPlugin();
   * plugin.run();
   * ```
   */
  run(): void {
    // The IPC setup in the constructor already listens on stdin.
    // The process stays alive until stdin closes or Shutdown is received.
    this.logger.info("Plugin process started");

    // Handle process signals
    process.on("SIGTERM", () => this.handleShutdown());
    process.on("SIGINT", () => this.handleShutdown());
  }

  // -----------------------------------------------------------------------
  // Internal
  // -----------------------------------------------------------------------

  private registerHandlers(): void {
    this.ipc.on("Init", async (msg) => {
      if (msg.type !== "Init") return;
      const { pluginName, pluginDir, permissions } = msg.payload;

      this.pluginName = pluginName;
      this.pluginDir = pluginDir;
      this.permissions = permissions as PluginPermission[];
      this.logger = new PluginLogger(pluginName);
      this.health = new HealthMonitor();

      this.logger.info("Received init", { pluginDir, permissions });

      try {
        await this.onInit();
        this.initialized = true;
        this.send({ type: "InitOk" });
        this.logger.info("Plugin initialized successfully");
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        this.logger.error("Init failed", { error: msg });
        this.sendError(undefined, `Init failed: ${msg}`);
      }
    });

    this.ipc.on("Translate", async (msg) => {
      if (msg.type !== "Translate") return;
      const { requestId, text, from, to } = msg.payload;

      this.logger.debug("Translate request", { requestId, from, to, textLen: text.length });

      try {
        const result = await this.onTranslate(requestId, text, from, to);
        this.health.recordRequest();
        this.send({
          type: "TranslateResult",
          payload: {
            requestId,
            result: { ok: result },
          },
        });
      } catch (err) {
        const errMsg = err instanceof Error ? err.message : String(err);
        this.health.recordError(errMsg);
        this.logger.error("Translation failed", { requestId, error: errMsg });
        this.send({
          type: "TranslateResult",
          payload: {
            requestId,
            result: { err: errMsg },
          },
        });
      }
    });

    this.ipc.on("Ping", (msg) => {
      if (msg.type !== "Ping") return;
      this.send({
        type: "Pong",
        payload: { requestId: msg.payload.requestId },
      });
    });

    this.ipc.on("Shutdown", () => {
      this.handleShutdown();
    });
  }

  private async handleShutdown(): Promise<void> {
    this.logger.info("Shutting down");
    try {
      await this.onShutdown();
    } catch (err) {
      this.logger.error("Shutdown error", { error: String(err) });
    }
    this.ipc.close();
    process.exit(0);
  }

  /** Send a message to the host */
  protected send(msg: PluginToHostMessage): void {
    this.ipc.send(msg);
  }

  /** Send an error message to the host */
  protected sendError(requestId: string | undefined, message: string): void {
    this.send({
      type: "Error",
      payload: { requestId, message },
    });
  }

  /** Check if the plugin has a specific permission */
  protected hasPermission(permission: PluginPermission): boolean {
    return this.permissions.includes(permission);
  }

  /** Assert a permission is available, throw if not */
  protected requirePermission(permission: PluginPermission): void {
    if (!this.hasPermission(permission)) {
      throw new Error(`Permission '${permission}' is required but not granted`);
    }
  }
}
