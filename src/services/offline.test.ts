import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getOfflineModels,
  downloadOfflineModel,
  deleteOfflineModel,
  getOfflineStatus,
} from './offline';
import { invokeOrThrow } from './invoke';

vi.mock('./invoke', () => ({
  invokeOrThrow: vi.fn(),
}));

describe('offline service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('getOfflineModels calls get_offline_models', async () => {
    vi.mocked(invokeOrThrow).mockResolvedValue([
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
    const r = await getOfflineModels();
    expect(invokeOrThrow).toHaveBeenCalledWith('get_offline_models');
    expect(r[0].id).toBe('en-zh');
  });

  it('downloadOfflineModel passes from/to', async () => {
    vi.mocked(invokeOrThrow).mockResolvedValue(undefined);
    await downloadOfflineModel('en', 'zh');
    expect(invokeOrThrow).toHaveBeenCalledWith('download_offline_model', {
      from: 'en',
      to: 'zh',
    });
  });

  it('deleteOfflineModel passes from/to', async () => {
    vi.mocked(invokeOrThrow).mockResolvedValue(undefined);
    await deleteOfflineModel('zh', 'en');
    expect(invokeOrThrow).toHaveBeenCalledWith('delete_offline_model', {
      from: 'zh',
      to: 'en',
    });
  });

  it('getOfflineStatus returns ready pairs', async () => {
    vi.mocked(invokeOrThrow).mockResolvedValue({
      enabled: true,
      autoSwitch: true,
      loadedModels: ['en-zh'],
      modelDir: 'C:/models',
    });
    const s = await getOfflineStatus();
    expect(invokeOrThrow).toHaveBeenCalledWith('get_offline_status');
    expect(s.loadedModels).toContain('en-zh');
  });
});
