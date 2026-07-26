import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import App from './App';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(),
}));

vi.mock('./pages/MainTranslator', () => ({
  default: () => <div data-testid="main-translator" />,
}));

vi.mock('./pages/Settings', () => ({
  default: () => <div />,
}));

vi.mock('./pages/DocumentsViewer', () => ({
  default: () => <div />,
}));

vi.mock('./pages/Vocabulary', () => ({
  default: () => <div />,
}));

vi.mock('./pages/Plugins', () => ({
  default: () => <div />,
}));

vi.mock('./pages/PluginMarketplace', () => ({
  default: () => <div />,
}));

vi.mock('./pages/MetricsDashboard', () => ({
  default: () => <div />,
}));

vi.mock('./pages/TmManager', () => ({
  default: () => <div />,
}));

vi.mock('./components/HookMonitor', () => ({
  default: () => <div />,
}));

vi.mock('./components/OcrScreenshotSelector', () => ({
  default: () => <div />,
}));

vi.mock('./components/OcrRegionFrame', () => ({
  default: () => <div />,
}));

vi.mock('./components/OcrScreenshotTranslator', () => ({
  default: () => null,
}));

vi.mock('./components/vocabulary', () => ({
  AIGenerationProgress: () => null,
}));

describe('App browser runtime', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
    vi.mocked(getCurrentWindow).mockReset();
  });

  test('does not call Tauri APIs when rendered outside the Tauri runtime', async () => {
    render(<App />);

    await waitFor(() => {
      expect(invoke).not.toHaveBeenCalled();
      expect(listen).not.toHaveBeenCalled();
      expect(getCurrentWindow).not.toHaveBeenCalled();
    });
  });
});
