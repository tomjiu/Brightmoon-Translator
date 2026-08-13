import { invokeOrThrow } from './invoke';
import { useConfigStore } from '../stores/configStore';

// Keep track of current audio to allow cancellation
let currentAudio: HTMLAudioElement | null = null;

/**
 * Speak text using the configured TTS provider.
 * When the provider is the local/system one, speechSynthesis (Web Speech API)
 * is used directly — a zero-config, offline fallback; otherwise the request
 * goes through the Rust backend.
 * Returns a promise that resolves when playback ends.
 * If called again while playing, the previous playback is stopped.
 */
export async function speakText(text: string, lang: string, voice?: string): Promise<void> {
  stopSpeaking();

  const provider = (useConfigStore.getState().config.ttsProvider || 'edge').trim().toLowerCase();
  if (provider === 'local' || provider === 'system') {
    return speakLocal(text, lang, voice);
  }

  try {
    await playRemote(text, lang, voice);
  } catch (e) {
    // Fall back to the local system voice when the remote backend fails
    // (e.g. no network, expired/missing key). This keeps TTS usable even
    // when the configured provider is unavailable.
    try {
      await speakLocal(text, lang, voice);
    } catch {
      throw e;
    }
  }
}

async function playRemote(text: string, lang: string, voice?: string): Promise<void> {
  const base64Audio = await invokeOrThrow<string>('text_to_speech', {
    text,
    lang,
    voice: voice || undefined,
  });
  const audioBytes = Uint8Array.from(atob(base64Audio), (c) => c.charCodeAt(0));
  const audioBlob = new Blob([audioBytes], { type: 'audio/mp3' });
  const audioUrl = URL.createObjectURL(audioBlob);
  const audio = new Audio(audioUrl);
  currentAudio = audio;

  return new Promise<void>((resolve) => {
    audio.onended = () => {
      currentAudio = null;
      URL.revokeObjectURL(audioUrl);
      resolve();
    };
    audio.onerror = () => {
      currentAudio = null;
      URL.revokeObjectURL(audioUrl);
      resolve();
    };
    audio.play();
  });
}

/**
 * Pick the best local voice for a language, honoring an explicit voice name.
 */
function pickLocalVoice(lang: string, voice?: string): SpeechSynthesisVoice | undefined {
  const voices = window.speechSynthesis.getVoices();
  if (voices.length === 0) return undefined;

  if (voice) {
    const byName = voices.find(
      (v) => v.name.toLowerCase() === voice.trim().toLowerCase(),
    );
    if (byName) return byName;
  }

  const langLower = lang.toLowerCase();
  const exact = voices.find((v) => v.lang.toLowerCase() === langLower);
  if (exact) return exact;
  const prefix = langLower.split('-')[0];
  const byPrefix = voices.find((v) => v.lang.toLowerCase().startsWith(prefix));
  if (byPrefix) return byPrefix;
  return voices.find((v) => v.lang) ?? voices[0];
}

function getSynth(): SpeechSynthesis | undefined {
  return typeof window !== 'undefined' ? window.speechSynthesis : undefined;
}

/**
 * List the local system voices exposed by speechSynthesis.
 * Returns [] when speechSynthesis is unavailable. Voices may load
 * asynchronously, so it retries briefly before giving up.
 */
export async function getLocalVoices(): Promise<SpeechSynthesisVoice[]> {
  const synth = getSynth();
  if (!synth) return [];

  const load = () => synth.getVoices();
  const first = load();
  if (first.length > 0) return first;

  await new Promise<void>((resolve) => {
    synth.addEventListener('voiceschanged', () => resolve(), { once: true });
    setTimeout(resolve, 1000);
  });
  return load();
}

/**
 * Speak text using the browser/system speech synthesis (Web Speech API).
 * Zero-config offline fallback; resolves when playback ends.
 * Throws if speechSynthesis is unavailable.
 */
export function speakLocal(text: string, lang: string, voice?: string): Promise<void> {
  const synth = getSynth();
  if (!synth) {
    return Promise.reject(new Error('speechSynthesis is not available'));
  }
  synth.cancel();

  const utter = new SpeechSynthesisUtterance(text);
  utter.lang = lang;
  const picked = pickLocalVoice(lang, voice);
  if (picked) utter.voice = picked;

  return new Promise<void>((resolve) => {
    utter.onend = () => resolve();
    utter.onerror = () => resolve();
    synth.speak(utter);
  });
}

/**
 * Stop any currently playing TTS audio (remote playback or local synthesis).
 */
export function stopSpeaking(): void {
  if (currentAudio) {
    currentAudio.pause();
    currentAudio.currentTime = 0;
    currentAudio = null;
  }
  getSynth()?.cancel();
}
