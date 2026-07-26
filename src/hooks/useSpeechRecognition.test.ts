import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSpeechRecognition } from './useSpeechRecognition';

interface MockSpeechRecognitionInstance {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  maxAlternatives: number;
  onresult: ((event: unknown) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  onstart: (() => void) | null;
  start: ReturnType<typeof vi.fn>;
  stop: ReturnType<typeof vi.fn>;
  abort: ReturnType<typeof vi.fn>;
}

describe('useSpeechRecognition', () => {
  let mockRecognition: MockSpeechRecognitionInstance;

  beforeEach(() => {
    vi.clearAllMocks();

    // Create fresh mock instance
    mockRecognition = {
      continuous: false,
      interimResults: false,
      lang: '',
      maxAlternatives: 1,
      onresult: null,
      onerror: null,
      onend: null,
      onstart: null,
      start: vi.fn(),
      stop: vi.fn(),
      abort: vi.fn(),
    };

    // Mock SpeechRecognition constructor
    (window as unknown as Record<string, unknown>).SpeechRecognition = vi.fn(() => mockRecognition);
  });

  describe('initial state', () => {
    it('should have correct initial state', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      expect(result.current.isListening).toBe(false);
      expect(result.current.interimTranscript).toBe('');
      expect(result.current.error).toBeNull();
      expect(result.current.isSupported).toBe(true);
    });
  });

  describe('startListening', () => {
    it('should start speech recognition', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      expect(mockRecognition.start).toHaveBeenCalled();
      expect(mockRecognition.continuous).toBe(true);
      expect(mockRecognition.interimResults).toBe(true);
    });

    it('should set language from parameter', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('zh');
      });

      expect(mockRecognition.lang).toBe('zh-CN');
    });

    it('should default to en-US for auto', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('auto');
      });

      expect(mockRecognition.lang).toBe('en-US');
    });

    it('should abort existing recognition before starting new', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      // Start first recognition
      act(() => {
        result.current.startListening('en');
      });

      const firstRecognition = mockRecognition;

      // Start second recognition
      act(() => {
        result.current.startListening('zh');
      });

      expect(firstRecognition.abort).toHaveBeenCalled();
    });

    it('should clear previous error on start', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      // Simulate error
      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'not-allowed' });
      });

      expect(result.current.error).toBeTruthy();

      // Start again
      act(() => {
        result.current.startListening('en');
      });

      expect(result.current.error).toBeNull();
    });
  });

  describe('stopListening', () => {
    it('should stop speech recognition', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        result.current.stopListening();
      });

      expect(mockRecognition.abort).toHaveBeenCalled();
      expect(result.current.isListening).toBe(false);
    });

    it('should clear interim transcript on stop', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      // Simulate interim result
      act(() => {
        mockRecognition.onresult?.({
          resultIndex: 0,
          results: [
            {
              isFinal: false,
              length: 1,
              item: () => ({ transcript: 'hello', confidence: 0.9 }),
              0: { transcript: 'hello', confidence: 0.9 },
            },
          ],
        });
      });

      expect(result.current.interimTranscript).toBe('hello');

      act(() => {
        result.current.stopListening();
      });

      expect(result.current.interimTranscript).toBe('');
    });
  });

  describe('onresult handler', () => {
    it('should accumulate final results', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      // Simulate final result
      act(() => {
        mockRecognition.onresult?.({
          resultIndex: 0,
          results: [
            {
              isFinal: true,
              length: 1,
              item: () => ({ transcript: 'Hello ', confidence: 0.95 }),
              0: { transcript: 'Hello ', confidence: 0.95 },
            },
          ],
        });
      });

      // Consume transcript
      const transcript = result.current.consumeTranscript();
      expect(transcript).toBe('Hello ');
    });

    it('should update interim transcript', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onresult?.({
          resultIndex: 0,
          results: [
            {
              isFinal: false,
              length: 1,
              item: () => ({ transcript: 'world', confidence: 0.8 }),
              0: { transcript: 'world', confidence: 0.8 },
            },
          ],
        });
      });

      expect(result.current.interimTranscript).toBe('world');
    });
  });

  describe('onerror handler', () => {
    it('should handle not-allowed error', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'not-allowed' });
      });

      expect(result.current.error).toBe('麦克风访问被拒绝，请允许麦克风权限');
    });

    it('should ignore no-speech error', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'no-speech' });
      });

      expect(result.current.error).toBeNull();
    });

    it('should handle network error', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'network' });
      });

      expect(result.current.error).toBe('网络错误，请检查网络连接');
    });

    it('should ignore aborted error', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'aborted' });
      });

      expect(result.current.error).toBeNull();
    });

    it('should handle other errors', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        mockRecognition.onerror?.({ error: 'unknown-error' });
      });

      expect(result.current.error).toBe('语音识别错误: unknown-error');
    });
  });

  describe('onend handler', () => {
    it('should auto-restart if still listening', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      // Simulate recognition end
      act(() => {
        mockRecognition.onend?.();
      });

      // Should create new recognition instance
      expect(window.SpeechRecognition).toHaveBeenCalledTimes(2);
    });

    it('should set isListening to false when stopped', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      act(() => {
        result.current.stopListening();
      });

      // Simulate recognition end after stop
      act(() => {
        mockRecognition.onend?.();
      });

      expect(result.current.isListening).toBe(false);
    });
  });

  describe('consumeTranscript', () => {
    it('should return accumulated transcript from single event', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      // Simulate a single result event with final result
      act(() => {
        mockRecognition.onresult?.({
          resultIndex: 0,
          results: [
            {
              isFinal: true,
              length: 1,
              item: () => ({ transcript: 'Hello World', confidence: 0.95 }),
              0: { transcript: 'Hello World', confidence: 0.95 },
            },
          ],
        });
      });

      const transcript = result.current.consumeTranscript();
      expect(transcript).toBe('Hello World');

      // Should be cleared after consume
      const emptyTranscript = result.current.consumeTranscript();
      expect(emptyTranscript).toBe('');
    });

    it('should accumulate multiple final results in same event', () => {
      const { result } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      // Simulate result event with multiple final results
      act(() => {
        mockRecognition.onresult?.({
          resultIndex: 0,
          results: [
            {
              isFinal: true,
              length: 1,
              item: () => ({ transcript: 'Hello ', confidence: 0.95 }),
              0: { transcript: 'Hello ', confidence: 0.95 },
            },
            {
              isFinal: true,
              length: 1,
              item: () => ({ transcript: 'World', confidence: 0.9 }),
              0: { transcript: 'World', confidence: 0.9 },
            },
          ],
        });
      });

      const transcript = result.current.consumeTranscript();
      expect(transcript).toBe('Hello World');
    });
  });

  describe('cleanup', () => {
    it('should abort recognition on unmount', () => {
      const { result, unmount } = renderHook(() => useSpeechRecognition());

      act(() => {
        result.current.startListening('en');
      });

      unmount();

      expect(mockRecognition.abort).toHaveBeenCalled();
    });
  });
});
