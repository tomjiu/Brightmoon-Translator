import { describe, it, expect, vi, beforeEach } from 'vitest';
import { speakText, stopSpeaking } from './tts';
import { invokeOrThrow } from './invoke';

// Mock invoke module
vi.mock('./invoke', () => ({
  invokeOrThrow: vi.fn(),
}));

interface MockAudioInstance {
  play: ReturnType<typeof vi.fn>;
  pause: ReturnType<typeof vi.fn>;
  currentTime: number;
  onended: (() => void) | null;
  onerror: (() => void) | null;
}

function createMockAudio(overrides: Partial<MockAudioInstance> = {}): MockAudioInstance {
  return {
    play: vi.fn(),
    pause: vi.fn(),
    currentTime: 0,
    onended: null,
    onerror: null,
    ...overrides,
  };
}

describe('tts service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    stopSpeaking();
  });

  describe('speakText', () => {
    it('should call invokeOrThrow with correct parameters', async () => {
      // Mock base64 audio response
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      // Mock Audio.play to trigger onended immediately
      const mockAudio = createMockAudio({
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          setTimeout(() => this.onended?.(), 0);
        }),
      });

      vi.spyOn(window, 'Audio').mockImplementation(() => mockAudio as unknown as HTMLAudioElement);

      await speakText('Hello', 'en');

      expect(invokeOrThrow).toHaveBeenCalledWith('text_to_speech', {
        text: 'Hello',
        lang: 'en',
        voice: undefined,
      });
    });

    it('should pass optional voice to invoke', async () => {
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      const mockAudio = createMockAudio({
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          setTimeout(() => this.onended?.(), 0);
        }),
      });

      vi.spyOn(window, 'Audio').mockImplementation(() => mockAudio as unknown as HTMLAudioElement);

      await speakText('Hello', 'en', 'en-US-GuyNeural');

      expect(invokeOrThrow).toHaveBeenCalledWith('text_to_speech', {
        text: 'Hello',
        lang: 'en',
        voice: 'en-US-GuyNeural',
      });
    });

    it('should resolve when audio playback ends', async () => {
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      const mockAudio = createMockAudio({
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          setTimeout(() => this.onended?.(), 10);
        }),
      });

      vi.spyOn(window, 'Audio').mockImplementation(() => mockAudio as unknown as HTMLAudioElement);

      await expect(speakText('Test', 'zh')).resolves.toBeUndefined();
    });

    it('should resolve on audio error', async () => {
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      const mockAudio = createMockAudio({
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          setTimeout(() => this.onerror?.(), 10);
        }),
      });

      vi.spyOn(window, 'Audio').mockImplementation(() => mockAudio as unknown as HTMLAudioElement);

      await expect(speakText('Test', 'en')).resolves.toBeUndefined();
    });

    it('should stop previous audio when called again', async () => {
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      const mockAudio1 = createMockAudio({ currentTime: 5 });

      const mockAudio2 = createMockAudio({
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          setTimeout(() => this.onended?.(), 10);
        }),
      });

      let callCount = 0;
      vi.spyOn(window, 'Audio').mockImplementation(() => {
        callCount++;
        return (callCount === 1 ? mockAudio1 : mockAudio2) as unknown as HTMLAudioElement;
      });

      // Start first playback (don't await - it won't resolve because play doesn't trigger onended)
      speakText('First', 'en');

      // Small delay to ensure first audio is set up
      await new Promise((r) => setTimeout(r, 20));

      // Start second playback - this should stop the first
      await speakText('Second', 'en');

      // First audio should have been paused by stopSpeaking()
      expect(mockAudio1.pause).toHaveBeenCalled();
    });
  });

  describe('stopSpeaking', () => {
    it('should stop currently playing audio', async () => {
      const mockBase64 = btoa('fake-audio-data');
      vi.mocked(invokeOrThrow).mockResolvedValue(mockBase64);

      const mockAudio = createMockAudio({
        currentTime: 5,
        play: vi.fn().mockImplementation(function (this: MockAudioInstance) {
          // Don't resolve - simulate ongoing playback
        }),
      });

      vi.spyOn(window, 'Audio').mockImplementation(() => mockAudio as unknown as HTMLAudioElement);

      // Start playback
      speakText('Test', 'en');

      // Wait a bit for the mock to be set up
      await new Promise((r) => setTimeout(r, 10));

      // Stop speaking
      stopSpeaking();

      expect(mockAudio.pause).toHaveBeenCalled();
      expect(mockAudio.currentTime).toBe(0);
    });

    it('should do nothing if no audio is playing', () => {
      // Should not throw
      expect(() => stopSpeaking()).not.toThrow();
    });
  });
});
