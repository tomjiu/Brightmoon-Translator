// Chrome API mock for testing browser extension code
// Provides mock implementations of chrome.runtime, chrome.storage, chrome.tabs, etc.

export function createChromeMock() {
  const listeners = new Map();
  const storage = new Map();
  const alarms = new Map();
  const contextMenus = new Map();
  const tabs = new Map();

  const mock = {
    runtime: {
      lastError: null,
      onMessage: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("runtime:message")) {
            listeners.set("runtime:message", []);
          }
          listeners.get("runtime:message").push(callback);
        }),
        removeListener: vi.fn(),
      },
      onInstalled: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("runtime:installed")) {
            listeners.set("runtime:installed", []);
          }
          listeners.get("runtime:installed").push(callback);
        }),
      },
      sendMessage: vi.fn((message, callback) => {
        // Simulate async message passing
        const messageListeners = listeners.get("runtime:message") || [];
        for (const listener of messageListeners) {
          const sendResponse = vi.fn((response) => {
            if (callback) callback(response);
          });
          const keepChannel = listener(message, { tab: { id: 1 } }, sendResponse);
          if (keepChannel) return; // async response
        }
        if (callback) callback({ success: false, error: "No listener" });
      }),
      getURL: vi.fn((path) => `chrome-extension://mock-id/${path}`),
      id: "mock-extension-id",
    },

    storage: {
      local: {
        get: vi.fn((key) => {
          if (typeof key === "string") {
            const value = storage.get(key);
            return Promise.resolve({ [key]: value });
          }
          if (Array.isArray(key)) {
            const result = {};
            for (const k of key) {
              result[k] = storage.get(k);
            }
            return Promise.resolve(result);
          }
          // Get all
          const result = {};
          for (const [k, v] of storage.entries()) {
            result[k] = v;
          }
          return Promise.resolve(result);
        }),
        set: vi.fn((items) => {
          for (const [key, value] of Object.entries(items)) {
            storage.set(key, value);
          }
          return Promise.resolve();
        }),
        remove: vi.fn((key) => {
          if (Array.isArray(key)) {
            key.forEach((k) => storage.delete(k));
          } else {
            storage.delete(key);
          }
          return Promise.resolve();
        }),
      },
      onChanged: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("storage:changed")) {
            listeners.set("storage:changed", []);
          }
          listeners.get("storage:changed").push(callback);
        }),
      },
    },

    tabs: {
      sendMessage: vi.fn((tabId, message) => {
        const tabListeners = listeners.get(`tabs:message:${tabId}`) || [];
        for (const listener of tabListeners) {
          listener(message, { tab: { id: tabId } }, vi.fn());
        }
        return Promise.resolve();
      }),
      query: vi.fn((queryInfo, callback) => {
        const mockTabs = [{ id: 1, url: "https://example.com", active: true }];
        if (callback) callback(mockTabs);
        return Promise.resolve(mockTabs);
      }),
    },

    alarms: {
      create: vi.fn((name, alarmInfo) => {
        alarms.set(name, alarmInfo);
      }),
      onAlarm: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("alarm")) {
            listeners.set("alarm", []);
          }
          listeners.get("alarm").push(callback);
        }),
      },
      _triggerAlarm: (name) => {
        const alarmListeners = listeners.get("alarm") || [];
        for (const listener of alarmListeners) {
          listener({ name });
        }
      },
    },

    contextMenus: {
      create: vi.fn((properties) => {
        contextMenus.set(properties.id, properties);
      }),
      onClicked: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("contextMenu:click")) {
            listeners.set("contextMenu:click", []);
          }
          listeners.get("contextMenu:click").push(callback);
        }),
      },
      _simulateClick: (menuItemId, info, tab) => {
        const clickListeners = listeners.get("contextMenu:click") || [];
        for (const listener of clickListeners) {
          listener({ menuItemId, ...info }, tab);
        }
      },
    },

    commands: {
      onCommand: {
        addListener: vi.fn((callback) => {
          if (!listeners.has("command")) {
            listeners.set("command", []);
          }
          listeners.get("command").push(callback);
        }),
      },
    },

    // Test helpers
    _listeners: listeners,
    _storage: storage,
    _alarms: alarms,
    _contextMenus: contextMenus,

    _reset: () => {
      listeners.clear();
      storage.clear();
      alarms.clear();
      contextMenus.clear();
      mock.runtime.lastError = null;
    },

    // Simulate storage change event
    _simulateStorageChange: (changes, area) => {
      const changeListeners = listeners.get("storage:changed") || [];
      for (const listener of changeListeners) {
        listener(changes, area);
      }
    },

    // Simulate incoming message
    _simulateMessage: (message, sender) => {
      return new Promise((resolve) => {
        const messageListeners = listeners.get("runtime:message") || [];
        for (const listener of messageListeners) {
          const sendResponse = (response) => resolve(response);
          listener(message, sender || { tab: { id: 1 } }, sendResponse);
        }
      });
    },
  };

  return mock;
}

// Install mock on global scope
export function installChromeMock() {
  const mock = createChromeMock();
  globalThis.chrome = mock;
  return mock;
}
