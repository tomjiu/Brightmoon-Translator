import { invokeOrThrow } from './invoke';

// Keep track of current audio to allow cancellation
let currentAudio: HTMLAudioElement | null = null;

/**
 * Speak text using Edge TTS.
 * Returns a promise that resolves when playback ends.
 * If called again while playing, the previous playback is stopped.
 */
export async function speakText(text: string, lang: string, voice?: string): Promise<void> {
  // Stop any currently playing audio
  stopSpeaking();

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
 * Stop any currently playing TTS audio.
 */
export function stopSpeaking(): void {
  if (currentAudio) {
    currentAudio.pause();
    currentAudio.currentTime = 0;
    currentAudio = null;
  }
}
