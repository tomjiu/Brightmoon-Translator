import { render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import WordBook from './WordBook';
// import Plugins from "./Plugins"; // removed
// import PluginMarketplace from "./PluginMarketplace";
import MetricsDashboard from './MetricsDashboard';
import TmManager from './TmManager';
import Settings from './Settings';
import { invokeOrThrow, safeInvoke } from '../services/invoke';
import { isEnabled } from '@tauri-apps/plugin-autostart';
import { useConfigStore } from '../stores/configStore';

vi.mock('../services/invoke', () => ({
  invokeOrThrow: vi.fn().mockRejectedValue(new Error('Tauri unavailable')),
  safeInvoke: vi.fn().mockResolvedValue([null, null]),
  invokeOrDefault: vi.fn().mockResolvedValue(0),
}));

vi.mock('@tauri-apps/plugin-autostart', () => ({
  enable: vi.fn(),
  disable: vi.fn(),
  isEnabled: vi.fn().mockResolvedValue(false),
}));

describe('data pages browser runtime', () => {
  beforeEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
    vi.mocked(invokeOrThrow).mockClear();
    vi.mocked(safeInvoke).mockClear();
    vi.mocked(isEnabled).mockClear();
    useConfigStore.setState({ loaded: false, cacheSize: 0 });
  });

  test.each([
    ['WordBook', () => <WordBook />],
    // ["Plugins", () => <Plugins />],
    // ["PluginMarketplace", () => <PluginMarketplace />],
    ['MetricsDashboard', () => <MetricsDashboard />],
    ['TmManager', () => <TmManager />],
    ['Settings', () => <Settings />],
  ])('%s does not load desktop backend data outside Tauri', async (_name, createElement) => {
    render(createElement());

    await waitFor(() => {
      expect(invokeOrThrow).not.toHaveBeenCalled();
      expect(safeInvoke).not.toHaveBeenCalled();
      expect(isEnabled).not.toHaveBeenCalled();
    });
  });
});
