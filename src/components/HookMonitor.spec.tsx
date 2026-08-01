import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import HookMonitor from './HookMonitor';
import { useConfigStore } from '../stores/configStore';
import type { AppConfig } from '../types';
import { listen } from '@tauri-apps/api/event';
import { showOverlayAt } from '../services/overlayPosition';
import { invokeOrThrow, safeInvoke } from '../services/invoke';

vi.mock('../services/invoke', () => ({
  safeInvoke: vi.fn().mockResolvedValue([null, null]),
  invokeOrThrow: vi.fn().mockResolvedValue(false),
}));

vi.mock('../services/tts', () => ({
  speakText: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../services/overlayPosition', () => ({
  showOverlayAt: vi.fn(),
  positionBelowText: vi.fn(() => ({ x: 10, y: 20, width: 300, height: 120 })),
  positionAtWindowBottom: vi.fn(() => ({ x: 10, y: 20, width: 300, height: 120 })),
}));

interface HookTranslatedPayload {
  window_title: string;
  process_name: string;
  original: string;
  translated: string;
  engine: string;
  timestamp: number;
  source: string;
  text_rect?: [number, number, number, number];
}

const baseConfig: AppConfig = {
  llm: { provider: 'deepseek', apiKey: '', apiKeys: [], baseUrl: '', model: '', providers: [] },
  engines: {
    google: { enabled: false },
    baidu: { enabled: false, appId: '', secret: '' },
    youdao: { enabled: false, useAi: false },
    deepl: { enabled: false, apiKey: '', pro: false },
    deeplx: { enabled: false, pro: false },
    microsoft: { enabled: false },
    yandex: { enabled: false },
    offline: { enabled: false, autoSwitch: true, downloadedModels: [], modelDir: '' },
  },
  defaultFrom: 'auto',
  defaultTo: 'zh',
  customPrompt: '',
  promptTemplates: [],
  llmTemperature: 0.3,
  llmMaxTokens: 4096,
  clipboardMonitor: false,
  useClipboardOutput: true,
  autoCopyResult: false,
  autoCopyMode: 'translated',
  translationMask: false,
  apiServerEnabled: false,
  apiServerPort: 60828,
  apiServerToken: '',
  hotkeys: {
    ocrTranslate: '',
    showWindow: '',
    translateSelection: '',
    replaceTranslate: '',
    toggleOverlayClickThrough: '',
  },
  proxy: { enabled: false, proxyType: 'http', host: '', port: 7890, username: '', password: '' },
  windowFollowMode: 'none',
  translationBlacklist: [],
  ocrEngine: 'auto',
  hook: {
    enabledSources: ['uia', 'clipboard', 'ocr', 'hook'],
    autoCopy: false,
    enabled: true,
    uiaIntervalMs: 500,
    ocrIntervalMs: 5000,
  },
  tmEnabled: false,
  tmThreshold: 0.8,
  cacheTtlHours: 72,
  furiganaEnabled: false,
  ttsAutoPlay: false,
  ttsVoice: '',
};

const emitHookTranslation = (payload: Partial<HookTranslatedPayload> = {}) => {
  const hookListener = vi
    .mocked(listen)
    .mock.calls.find(([eventName]) => eventName === 'hook-text-translated')?.[1];

  expect(hookListener).toBeDefined();

  act(() => {
    hookListener?.({
      event: 'hook-text-translated',
      id: 1,
      payload: {
        window_title: 'Example',
        process_name: 'example.exe',
        original: 'hello',
        translated: '你好',
        engine: 'test',
        timestamp: 1_700_000_000_000,
        source: 'uia',
        text_rect: [100, 200, 300, 40],
        ...payload,
      },
    });
  });
};

describe('HookMonitor overlay defaults', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      configurable: true,
    });
    Element.prototype.scrollIntoView = vi.fn();
    vi.mocked(listen).mockResolvedValue(vi.fn());
    useConfigStore.setState({
      config: baseConfig,
      loaded: true,
      saved: false,
      cacheSize: 0,
      cacheStats: null,
    });
  });

  it('does not show overlay when hook config has not explicitly enabled it', async () => {
    render(<HookMonitor />);

    await waitFor(() => {
      expect(listen).toHaveBeenCalledWith('hook-text-translated', expect.any(Function));
    });

    emitHookTranslation();

    expect(showOverlayAt).not.toHaveBeenCalled();
  });

  it('shows overlay when hook config explicitly enables it', async () => {
    useConfigStore.setState({
      config: {
        ...baseConfig,
        hook: { ...baseConfig.hook, showOverlay: true },
      },
    });

    render(<HookMonitor />);

    await waitFor(() => {
      expect(listen).toHaveBeenCalledWith('hook-text-translated', expect.any(Function));
    });

    emitHookTranslation();

    expect(showOverlayAt).toHaveBeenCalledWith(
      { x: 10, y: 20, width: 300, height: 120 },
      '你好',
      'hello',
    );
  });
});

describe('HookMonitor browser runtime', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    Element.prototype.scrollIntoView = vi.fn();
    useConfigStore.setState({
      config: baseConfig,
      loaded: true,
      saved: false,
      cacheSize: 0,
      cacheStats: null,
    });
  });

  it('does not call Tauri APIs on mount outside the Tauri runtime', async () => {
    render(<HookMonitor />);

    await waitFor(() => {
      expect(listen).not.toHaveBeenCalled();
      expect(invokeOrThrow).not.toHaveBeenCalledWith('get_hook_monitor_status');
      expect(safeInvoke).not.toHaveBeenCalledWith('hook_status', undefined, { silent: true });
    });
  });
});
