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

  describe('speakLocal', () => {
    const speakLocal = async (text: string, lang: string, voice?: string) => {
      // Re-import to keep test isolated from module-level state
      const mod = await import('./tts');
      return mod.speakLocal(text, lang, voice);
    };

    it('should reject when speechSynthesis is unavailable', async () => {
      // jsdom does not provide speechSynthesis by default
      Object.defineProperty(window, 'speechSynthesis', { value: undefined, configurable: true });
      await expect(speakLocal('hi', 'en')).rejects.toThrow('speechSynthesis is not available');
    });

    it('should speak via speechSynthesis and resolve on end', async () => {
      const mockUtter = {
        lang: '',
        voice: undefined,
        onend: null,
        onerror: null,
      };
      const mockSynth = {
        cancel: vi.fn(),
        getVoices: vi.fn().mockReturnValue([
          { name: 'Google US English', lang: 'en-US', voiceURI: 'x', default: true, localService: true },
        ]),
        speak: vi.fn().mockImplementation(function (this: unknown, u: typeof mockUtter) {
          (u as { onend: (() => void) | null }).onend?.();
        }),
      };
      Object.defineProperty(window, 'speechSynthesis', { value: mockSynth, configurable: true });
      vi.stubGlobal('SpeechSynthesisUtterance', class {
        lang = '';
        voice = undefined;
        onend: (() => void) | null = null;
        onerror: (() => void) | null = null;
        constructor(public text: string) {}
      });

      await expect(speakLocal('hi', 'en')).resolves.toBeUndefined();
      expect(mockSynth.cancel).toHaveBeenCalled();
      expect(mockSynth.speak).toHaveBeenCalled();

      vi.unstubAllGlobals();
      Object.defineProperty(window, 'speechSynthesis', { value: undefined, configurable: true });
    });

    it('should cancel previous synthesis on stopSpeaking', () => {
      const mockSynth = { cancel: vi.fn(), getVoices: vi.fn().mockReturnValue([]), speak: vi.fn() };
      Object.defineProperty(window, 'speechSynthesis', { value: mockSynth, configurable: true });
      stopSpeaking();
      expect(mockSynth.cancel).toHaveBeenCalled();
      Object.defineProperty(window, 'speechSynthesis', { value: undefined, configurable: true });
    });

    it('should fall back to local speech when the remote backend fails', async () => {
      vi.mocked(invokeOrThrow).mockRejectedValue(new Error('backend down'));

      const mockUtter = { lang: '', voice: undefined, onend: null, onerror: null };
      const mockSynth = {
        cancel: vi.fn(),
        getVoices: vi.fn().mockReturnValue([]),
        speak: vi.fn().mockImplementation(function (this: unknown, u: typeof mockUtter) {
          (u as { onend: (() => void) | null }).onend?.();
        }),
      };
      Object.defineProperty(window, 'speechSynthesis', { value: mockSynth, configurable: true });
      vi.stubGlobal('SpeechSynthesisUtterance', class {
        lang = '';
        voice = undefined;
        onend: (() => void) | null = null;
        onerror: (() => void) | null = null;
        constructor(public text: string) {}
      });

      // Use the re-imported module's speakText so provider defaults to 'edge'
      // (remote path) and the mocked invoke fails -> should fall back to local.
      const mod = await import('./tts');
      await expect(mod.speakText('hi', 'en')).resolves.toBeUndefined();

      expect(mockSynth.speak).toHaveBeenCalled();
      expect(mockSynth.cancel).toHaveBeenCalled();

      vi.unstubAllGlobals();
      Object.defineProperty(window, 'speechSynthesis', { value: undefined, configurable: true });
      vi.mocked(invokeOrThrow).mockResolvedValue(btoa('fake-audio-data'));
    });
  });
});
