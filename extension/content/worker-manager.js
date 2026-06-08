// Worker Manager for Moon Translator
// Bridges Web Worker with chrome.runtime messaging

(function() {
  "use strict";

  class TranslationWorkerManager {
    constructor() {
      this.worker = null;
      this.pendingCallbacks = new Map();
      this.isReady = false;
      this.initPromise = this.init();
    }

    async init() {
      try {
        // Create worker from extension URL
        const workerUrl = chrome.runtime.getURL("content/translation-worker.js");
        this.worker = new Worker(workerUrl);

        // Set up message handler
        this.worker.onmessage = (e) => this.handleWorkerMessage(e.data);
        this.worker.onerror = (e) => {
          console.error("Translation worker error:", e);
          this.isReady = false;
        };

        // Wait for worker ready signal
        await new Promise((resolve) => {
          const readyHandler = (e) => {
            if (e.data.type === "ready") {
              this.worker.removeEventListener("message", readyHandler);
              this.isReady = true;
              resolve();
            }
          };
          this.worker.addEventListener("message", readyHandler);
        });
      } catch (error) {
        console.error("Failed to initialize translation worker:", error);
        this.isReady = false;
      }
    }

    handleWorkerMessage(data) {
      const { type, id, payload, parentId } = data;

      switch (type) {
        case "requestTranslation":
          this.handleTranslationRequest(id, payload);
          break;

        case "requestBatchTranslation":
          this.handleBatchTranslationRequest(id, parentId, payload);
          break;

        case "translationResponse":
        case "translationErrorResponse":
          // Forward response to worker
          this.worker.postMessage({ type, id, payload });
          break;

        case "progress":
          // Notify progress listeners
          if (this.onProgress) {
            this.onProgress(parentId || id, payload);
          }
          break;

        case "batchTranslationComplete":
        case "translationError":
          // Resolve/reject pending promise
          const callback = this.pendingCallbacks.get(id);
          if (callback) {
            this.pendingCallbacks.delete(id);
            if (type === "batchTranslationComplete") {
              callback.resolve(payload);
            } else {
              callback.reject(new Error(payload.error));
            }
          }
          break;
      }
    }

    async handleTranslationRequest(id, payload) {
      try {
        const response = await this.sendToBackground({
          type: "translate",
          text: payload.text,
          from: payload.from,
          to: payload.to
        });

        this.worker.postMessage({
          type: "translationResponse",
          id,
          payload: response
        });
      } catch (error) {
        this.worker.postMessage({
          type: "translationErrorResponse",
          id,
          payload: { error: error.message }
        });
      }
    }

    async handleBatchTranslationRequest(id, parentId, payload) {
      try {
        const results = [];
        for (const text of payload.texts) {
          try {
            const response = await this.sendToBackground({
              type: "translate",
              text,
              from: payload.from,
              to: payload.to
            });
            results.push(response);
          } catch (e) {
            results.push({ error: e.message });
          }
        }

        this.worker.postMessage({
          type: "translationResponse",
          id,
          payload: { success: true, results }
        });
      } catch (error) {
        this.worker.postMessage({
          type: "translationErrorResponse",
          id,
          payload: { error: error.message }
        });
      }
    }

    sendToBackground(message) {
      return new Promise((resolve, reject) => {
        try {
          chrome.runtime.sendMessage(message, (response) => {
            if (chrome.runtime.lastError) {
              reject(new Error(chrome.runtime.lastError.message));
            } else if (response?.success) {
              resolve(response);
            } else {
              reject(new Error(response?.error || "Translation failed"));
            }
          });
        } catch (e) {
          reject(e);
        }
      });
    }

    // Public API: Translate single text
    async translate(text, from, to) {
      await this.initPromise;
      if (!this.isReady) {
        throw new Error("Worker not initialized");
      }

      const id = `translate_${++requestIdCounter}`;
      return new Promise((resolve, reject) => {
        this.pendingCallbacks.set(id, { resolve, reject });
        this.worker.postMessage({
          type: "translate",
          id,
          payload: { text, from, to }
        });
      });
    }

    // Public API: Batch translate
    async batchTranslate(texts, from, to, batchSize = 3) {
      await this.initPromise;
      if (!this.isReady) {
        throw new Error("Worker not initialized");
      }

      const id = `batch_${++requestIdCounter}`;
      return new Promise((resolve, reject) => {
        this.pendingCallbacks.set(id, { resolve, reject });
        this.worker.postMessage({
          type: "batchTranslate",
          id,
          payload: { texts, from, to, batchSize }
        });
      });
    }

    // Cancel pending request
    cancel(id) {
      this.worker.postMessage({ type: "cancel", id });
      this.pendingCallbacks.delete(id);
    }

    // Terminate worker
    terminate() {
      if (this.worker) {
        this.worker.terminate();
        this.worker = null;
        this.isReady = false;
      }
    }
  }

  // Counter for unique request IDs
  let requestIdCounter = 0;

  // Export as global
  window.MoonWorkerManager = new TranslationWorkerManager();
})();
