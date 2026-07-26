import { describe, it, expect, vi, beforeEach } from 'vitest';
import { safeInvoke, invokeOrThrow, invokeOrDefault } from './invoke';
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { useToastStore } from '../stores/toastStore';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('invoke service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useToastStore.setState({ toasts: [] });
  });

  describe('safeInvoke', () => {
    it('should return data on success', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue({ result: 'success' });

      const [data, error] = await safeInvoke('test_command');

      expect(data).toEqual({ result: 'success' });
      expect(error).toBeNull();
    });

    it('should return error on failure', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Command failed'));

      const [data, error] = await safeInvoke('test_command');

      expect(data).toBeNull();
      expect(error).toEqual({
        code: 'UNKNOWN',
        message: 'Command failed',
      });
    });

    it('should pass arguments to invoke', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue('ok');

      await safeInvoke('test_command', { arg1: 'value1', arg2: 42 });

      expect(tauriInvoke).toHaveBeenCalledWith('test_command', {
        arg1: 'value1',
        arg2: 42,
      });
    });

    it('should handle string errors', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue('String error');

      const [data, error] = await safeInvoke('test_command');

      expect(data).toBeNull();
      expect(error).toEqual({
        code: 'UNKNOWN',
        message: 'String error',
      });
    });

    it('should handle error objects with message', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue({
        message: 'Tauri error',
        code: 'TAURI_ERR',
      });

      const [data, error] = await safeInvoke('test_command');

      expect(data).toBeNull();
      expect(error).toEqual({
        code: 'TAURI_ERR',
        message: 'Tauri error',
      });
    });

    it('should handle error objects with detail', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue({
        message: 'Error with detail',
        code: 'ERR',
        detail: 'Detailed info',
      });

      const [data, error] = await safeInvoke('test_command');

      expect(data).toBeNull();
      expect(error).toEqual({
        code: 'ERR',
        message: 'Error with detail',
        detail: 'Detailed info',
      });
    });

    it('should not log error when silent option is true', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Silent error'));

      await safeInvoke('test_command', undefined, { silent: true });

      expect(consoleSpy).not.toHaveBeenCalled();
      consoleSpy.mockRestore();
    });

    it('should log error by default', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Logged error'));

      await safeInvoke('test_command');

      expect(consoleSpy).toHaveBeenCalledWith(
        '[invoke] test_command failed:',
        expect.objectContaining({ message: 'Logged error' }),
      );
      consoleSpy.mockRestore();
    });
  });

  describe('invokeOrThrow', () => {
    it('should return data on success', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue('success');

      const result = await invokeOrThrow('test_command');

      expect(result).toBe('success');
    });

    it('should throw on error', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Command failed'));

      await expect(invokeOrThrow('test_command')).rejects.toThrow('Command failed');
    });

    it('should show toast on error', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Toast error'));

      try {
        await invokeOrThrow('test_command');
      } catch {
        // Expected
      }

      const { toasts } = useToastStore.getState();
      expect(toasts).toHaveLength(1);
      expect(toasts[0].type).toBe('error');
      expect(toasts[0].message).toBe('Toast error');
    });
  });

  describe('invokeOrDefault', () => {
    it('should return data on success', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue(42);

      const result = await invokeOrDefault('test_command', undefined, 0);

      expect(result).toBe(42);
    });

    it('should return default value on error', async () => {
      vi.mocked(tauriInvoke).mockRejectedValue(new Error('Failed'));

      const result = await invokeOrDefault('test_command', undefined, 99);

      expect(result).toBe(99);
    });

    it('should return default value when data is null', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue(null);

      const result = await invokeOrDefault('test_command', undefined, 'default');

      expect(result).toBe('default');
    });

    it('should pass arguments to invoke', async () => {
      vi.mocked(tauriInvoke).mockResolvedValue('ok');

      await invokeOrDefault('test_command', { key: 'value' }, 'default');

      expect(tauriInvoke).toHaveBeenCalledWith('test_command', { key: 'value' });
    });
  });
});
