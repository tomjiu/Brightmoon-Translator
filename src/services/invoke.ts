import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { useToastStore } from '../stores/toastStore';

interface InvokeError {
  code: string;
  message: string;
  detail?: string;
}

/**
 * Safe wrapper for Tauri invoke that provides consistent error handling.
 * Returns a result tuple: [data, null] on success, [null, error] on failure.
 */
export async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options?: { silent?: boolean },
): Promise<[T | null, InvokeError | null]> {
  try {
    const data = await tauriInvoke<T>(command, args);
    return [data, null];
  } catch (err) {
    const error = parseError(err);
    if (!options?.silent) {
      console.error(`[invoke] ${command} failed:`, error);
    }
    return [null, error];
  }
}

/**
 * Parse various error formats into a consistent structure.
 */
function parseError(err: unknown): InvokeError {
  if (typeof err === 'string') {
    return { code: 'UNKNOWN', message: err };
  }

  if (err && typeof err === 'object') {
    // Tauri error object
    if ('message' in err && typeof err.message === 'string') {
      return {
        code: 'code' in err && typeof err.code === 'string' ? err.code : 'UNKNOWN',
        message: err.message,
        detail: 'detail' in err && typeof err.detail === 'string' ? err.detail : undefined,
      };
    }

    // Error instance
    if (err instanceof Error) {
      return { code: 'UNKNOWN', message: err.message };
    }
  }

  return { code: 'UNKNOWN', message: String(err) };
}

/**
 * Invoke with automatic error toast display.
 * Returns data on success, shows toast and throws on failure.
 * Use safeInvoke directly if you need custom error handling without the auto-toast.
 */
export async function invokeOrThrow<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const [data, error] = await safeInvoke<T>(command, args);
  if (error) {
    useToastStore.getState().addToast({
      type: 'error',
      message: error.message,
      duration: 4000,
    });
    throw new Error(error.message);
  }
  return data!;
}

/**
 * Invoke that returns a default value on error.
 */
export async function invokeOrDefault<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  defaultValue: T,
): Promise<T> {
  const [data] = await safeInvoke<T>(command, args, { silent: true });
  return data ?? defaultValue;
}
