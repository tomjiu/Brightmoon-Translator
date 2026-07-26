import { describe, expect, it, vi, beforeEach } from 'vitest';
import { WindowBindingManager, FOLLOW_POLL_MS } from './ocrWindowBinding';

vi.mock('../services/invoke', () => ({
  safeInvoke: vi.fn(),
}));

import { safeInvoke } from '../services/invoke';

describe('WindowBindingManager', () => {
  beforeEach(() => {
    vi.mocked(safeInvoke).mockReset();
  });

  it('exports follow poll ≤ 50ms (I6)', () => {
    expect(FOLLOW_POLL_MS).toBeLessThanOrEqual(50);
  });

  it('refuses to bind OCR chrome window titles (smoke #9)', async () => {
    vi.mocked(safeInvoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'hwnd_from_point') return [42, null];
      if (cmd === 'detect_foreground_hwnd') return [42, null];
      if (cmd === 'get_window_rect_cmd') return [{ x: 0, y: 0, width: 800, height: 600 }, null];
      if (cmd === 'get_window_title_cmd') return ['OCR Region', null];
      return [null, { code: 'X', message: 'no' }];
    });

    const mgr = new WindowBindingManager({
      onRegionUpdate: () => undefined,
      onWindowMinimized: () => undefined,
      onWindowRestored: () => undefined,
      onOverlayPositionSync: () => undefined,
    });
    const bound = await mgr.bind({ x: 10, y: 10, width: 100, height: 50 });
    expect(bound).toBeNull();
    mgr.dispose();
  });

  it('binds a normal window and stores offset', async () => {
    vi.mocked(safeInvoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'hwnd_from_point') return [99, null];
      if (cmd === 'get_window_rect_cmd') return [{ x: 100, y: 200, width: 800, height: 600 }, null];
      if (cmd === 'get_window_title_cmd') return ['Notepad', null];
      return [null, { code: 'X', message: 'no' }];
    });

    const mgr = new WindowBindingManager({
      onRegionUpdate: () => undefined,
      onWindowMinimized: () => undefined,
      onWindowRestored: () => undefined,
      onOverlayPositionSync: () => undefined,
    });
    const bound = await mgr.bind({ x: 150, y: 250, width: 200, height: 80 });
    expect(bound).not.toBeNull();
    expect(bound?.hwnd).toBe(99);
    expect(bound?.offset.dx).toBe(50);
    expect(bound?.offset.dy).toBe(50);
    mgr.unbind();
    mgr.dispose();
  });
});
