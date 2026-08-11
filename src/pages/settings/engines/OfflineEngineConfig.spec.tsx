import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { listen } from '@tauri-apps/api/event';
import OfflineEngineConfig from './OfflineEngineConfig';
import type { AppConfig } from '../../../types';
import {
  deleteOfflineModel,
  downloadOfflineModel,
  getOfflineModels,
} from '../../../services/offline';

vi.mock('../../../services/offline', () => ({
  getOfflineModels: vi.fn(),
  downloadOfflineModel: vi.fn(),
  deleteOfflineModel: vi.fn(),
  getOfflineStatus: vi.fn(),
}));

vi.mock('../../../i18n', () => ({
  useI18n: () => ({ t: (k: string) => k }),
}));

const baseConfig: AppConfig = {
  llm: { provider: 'deepseek', apiKey: '', apiKeys: [], baseUrl: '', model: '', providers: [] },
  engines: {
    google: { enabled: false },
    baidu: { enabled: false, appId: '', secret: '' },
    youdao: { enabled: false, useAi: false, ocrAppKey: '', ocrAppSecret: '' },
    deepl: { enabled: false, apiKey: '', pro: false },
    deeplx: { enabled: false, pro: false },
    caiyun: { enabled: false, apiToken: '' },
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

const makeProps = () => ({
  config: baseConfig,
  updateConfig: vi.fn(),
  saveConfig: vi.fn(),
});

describe('OfflineEngineConfig', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listen).mockResolvedValue(vi.fn());
  });

  it('lists language pairs with display names, not raw model ids', async () => {
    vi.mocked(getOfflineModels).mockResolvedValue([
      {
        id: 'en-zh',
        from: 'en',
        to: 'zh',
        displayName: 'English → Chinese',
        sizeLabel: '49MB',
        sizeBytes: 51e6,
        downloaded: false,
        sha256: 'a'.repeat(64),
      },
      {
        id: 'zh-en',
        from: 'zh',
        to: 'en',
        displayName: 'Chinese → English',
        sizeLabel: '70MB',
        sizeBytes: 73e6,
        downloaded: true,
        sha256: 'b'.repeat(64),
      },
    ]);

    render(<OfflineEngineConfig {...makeProps()} />);

    await waitFor(() =>
      expect(screen.getByText('English → Chinese')).toBeInTheDocument(),
    );
    expect(screen.getByText('Chinese → English')).toBeInTheDocument();
    expect(screen.queryByText(/model\.enzh|intgemm/)).toBeNull();
  });

  it('downloads the pair on click', async () => {
    vi.mocked(getOfflineModels).mockResolvedValue([
      {
        id: 'en-zh',
        from: 'en',
        to: 'zh',
        displayName: 'English → Chinese',
        sizeLabel: '49MB',
        sizeBytes: 51e6,
        downloaded: false,
        sha256: 'a'.repeat(64),
      },
    ]);

    render(<OfflineEngineConfig {...makeProps()} />);

    await waitFor(() => screen.getByText('English → Chinese'));
    fireEvent.click(screen.getByText('settings.enginePage.download'));
    await waitFor(() => expect(downloadOfflineModel).toHaveBeenCalledWith('en', 'zh'));
  });

  it('deletes the pair on click', async () => {
    vi.mocked(getOfflineModels).mockResolvedValue([
      {
        id: 'zh-en',
        from: 'zh',
        to: 'en',
        displayName: 'Chinese → English',
        sizeLabel: '70MB',
        sizeBytes: 73e6,
        downloaded: true,
        sha256: 'b'.repeat(64),
      },
    ]);

    render(<OfflineEngineConfig {...makeProps()} />);

    await waitFor(() => screen.getByText('Chinese → English'));
    fireEvent.click(screen.getByText('settings.enginePage.delete'));
    await waitFor(() => expect(deleteOfflineModel).toHaveBeenCalledWith('zh', 'en'));
  });
});
