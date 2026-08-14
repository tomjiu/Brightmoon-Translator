import { useState, useRef, useCallback, useEffect } from 'react';
import { useI18n } from '../i18n';
import { listen } from '@tauri-apps/api/event';
import { isTauriRuntime } from '../services/tauriRuntime';

interface SpeechRecognitionHook {
  isListening: boolean;
  interimTranscript: string;
  error: string | null;
  startListening: (lang?: string) => void;
  stopListening: () => void;
  isSupported: boolean;
  /** Call this to consume accumulated final transcript */
  consumeTranscript: () => string;
}

// Language code to BCP 47 tag mapping
const LANG_TO_BCP47: Record<string, string> = {
  zh: 'zh-CN',
  en: 'en-US',
  ja: 'ja-JP',
  ko: 'ko-KR',
  fr: 'fr-FR',
  de: 'de-DE',
  es: 'es-ES',
  ru: 'ru-RU',
  pt: 'pt-BR',
  it: 'it-IT',
  ar: 'ar-SA',
  th: 'th-TH',
  vi: 'vi-VN',
  auto: 'en-US',
};

// Type definition for SpeechRecognition
interface SpeechRecognitionEvent {
  resultIndex: number;
  results: SpeechRecognitionResultList;
}

interface SpeechRecognitionResultList {
  length: number;
  item(index: number): SpeechRecognitionResult;
  [index: number]: SpeechRecognitionResult;
}

interface SpeechRecognitionResult {
  isFinal: boolean;
  length: number;
  item(index: number): SpeechRecognitionAlternative;
  [index: number]: SpeechRecognitionAlternative;
}

interface SpeechRecognitionAlternative {
  transcript: string;
  confidence: number;
}

interface SpeechRecognitionInstance {
  continuous: boolean;
  interimResults: boolean;
  lang: string;
  maxAlternatives: number;
  start: () => void;
  stop: () => void;
  abort: () => void;
  onresult: ((event: SpeechRecognitionEvent) => void) | null;
  onerror: ((event: { error: string }) => void) | null;
  onend: (() => void) | null;
  onstart: (() => void) | null;
}

declare global {
  interface Window {
    SpeechRecognition?: new () => SpeechRecognitionInstance;
    webkitSpeechRecognition?: new () => SpeechRecognitionInstance;
  }
}

// Check support once
const getSpeechRecognitionAPI = (): (new () => SpeechRecognitionInstance) | null => {
  if (typeof window === 'undefined') return null;
  return window.SpeechRecognition ?? window.webkitSpeechRecognition ?? null;
};

// Native (Tauri) recognition events
interface NativeResultPayload {
  text: string;
  confidence: number;
  isFinal: boolean;
}

interface NativeStatusPayload {
  isListening: boolean;
  language: string;
  error?: string | null;
}

export function useSpeechRecognition(): SpeechRecognitionHook {
  const [isListening, setIsListening] = useState(false);
  const [interimTranscript, setInterimTranscript] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Use ref for accumulated transcript to avoid state timing issues
  const accumulatedTranscriptRef = useRef('');
  const recognitionRef = useRef<SpeechRecognitionInstance | null>(null);
  const isListeningRef = useRef(false);
  const langRef = useRef('en-US');

  const isTauri = isTauriRuntime();
  const isSupported = !!getSpeechRecognitionAPI() || isTauri;

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (recognitionRef.current) {
        isListeningRef.current = false;
        recognitionRef.current.abort();
        recognitionRef.current = null;
      }
    };
  }, []);

  // Native event listeners
  useEffect(() => {
    if (!isTauri) return;

    const unlisteners: Array<() => void> = [];

    const setup = async () => {
      const t = useI18n.getState().t;
      unlisteners.push(
        await listen<NativeResultPayload>('speech-recognition-result', (event) => {
          const payload = event.payload;
          const text = payload.text || '';
          if (!text.trim()) return;
          if (payload.isFinal) {
            accumulatedTranscriptRef.current += text;
            setInterimTranscript('');
          } else {
            setInterimTranscript(text);
          }
        }),
      );

      unlisteners.push(
        await listen('speech-recognition-start', () => {
          setIsListening(true);
          isListeningRef.current = true;
        }),
      );

      unlisteners.push(
        await listen('speech-recognition-stop', () => {
          setIsListening(false);
          isListeningRef.current = false;
          setInterimTranscript('');
        }),
      );

      unlisteners.push(
        await listen<NativeStatusPayload>('speech-recognition-error', (event) => {
          const msg = event.payload.error ?? '';
          if (msg.includes('access') || msg.includes('denied') || msg.includes('E_ACCESSDENIED')) {
            setError(t('speech.micDenied'));
          } else if (msg.includes('NoMatch')) {
            // No speech detected, keep listening
          } else {
            setError(t('speech.error', { error: msg }));
          }
          setIsListening(false);
          isListeningRef.current = false;
        }),
      );
    };

    void setup();
    return () => {
      for (const fn of unlisteners) fn();
    };
  }, [isTauri]);

  const startNativeRecognition = useCallback(async (lang: string) => {
    const { invokeOrThrow } = await import('../services/invoke');
    setError(null);
    setInterimTranscript('');
    accumulatedTranscriptRef.current = '';
    langRef.current = LANG_TO_BCP47[lang] || LANG_TO_BCP47.en;

    // Optimistically show listening; native start event will confirm.
    isListeningRef.current = true;
    setIsListening(true);

    try {
      await invokeOrThrow('start_speech_recognition', { lang: langRef.current });
    } catch (err) {
      isListeningRef.current = false;
      setIsListening(false);
      const message = err instanceof Error ? err.message : String(err);
      setError(
        message.includes('denied') || message.includes('access')
          ? useI18n.getState().t('speech.micDenied')
          : useI18n.getState().t('speech.startFailed'),
      );
    }
  }, []);

  const stopNativeRecognition = useCallback(async () => {
    isListeningRef.current = false;
    setIsListening(false);
    setInterimTranscript('');
    const { invokeOrDefault } = await import('../services/invoke');
    await invokeOrDefault('stop_speech_recognition', undefined, undefined);
  }, []);

  const createAndStartRecognition = useCallback(() => {
    const SpeechRecognitionAPI = getSpeechRecognitionAPI();
    if (!SpeechRecognitionAPI) return null;

    const recognition = new SpeechRecognitionAPI();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = langRef.current;
    recognition.maxAlternatives = 1;

    recognition.onstart = () => {
      setIsListening(true);
    };

    recognition.onresult = (event: SpeechRecognitionEvent) => {
      let interim = '';

      for (let i = event.resultIndex; i < event.results.length; i++) {
        const result = event.results[i];
        if (result.isFinal) {
          // Accumulate final results in ref (no state update needed)
          accumulatedTranscriptRef.current += result[0].transcript;
        } else {
          interim += result[0].transcript;
        }
      }

      setInterimTranscript(interim);
    };

    recognition.onerror = (event: { error: string }) => {
      const t = useI18n.getState().t;
      console.error('Speech recognition error:', event.error);
      if (event.error === 'not-allowed') {
        setError(t('speech.micDenied'));
        isListeningRef.current = false;
      } else if (event.error === 'no-speech') {
        // No speech detected, continue
      } else if (event.error === 'network') {
        setError(t('speech.networkError'));
      } else if (event.error === 'aborted') {
        // Intentional abort, ignore
      } else {
        setError(t('speech.error', { error: event.error }));
      }
    };

    recognition.onend = () => {
      // Auto-restart if we're still supposed to be listening
      if (isListeningRef.current) {
        // Create a new instance for reliability
        const newRecognition = createAndStartRecognition();
        if (newRecognition) {
          recognitionRef.current = newRecognition;
        } else {
          setIsListening(false);
          isListeningRef.current = false;
        }
      } else {
        setIsListening(false);
        setInterimTranscript('');
      }
    };

    try {
      recognition.start();
      return recognition;
    } catch (err) {
      console.error('Failed to start speech recognition:', err);
      setError(useI18n.getState().t('speech.startFailed'));
      return null;
    }
  }, []);

  const startListening = useCallback(
    (lang = 'auto') => {
      // Stop any existing recognition
      if (recognitionRef.current) {
        isListeningRef.current = false;
        recognitionRef.current.abort();
        recognitionRef.current = null;
      }

      setError(null);
      setInterimTranscript('');
      accumulatedTranscriptRef.current = '';
      langRef.current = LANG_TO_BCP47[lang] || LANG_TO_BCP47.en;
      isListeningRef.current = true;

      if (isTauri) {
        void startNativeRecognition(lang);
        return;
      }

      const recognition = createAndStartRecognition();
      if (recognition) {
        recognitionRef.current = recognition;
      } else {
        isListeningRef.current = false;
      }
    },
    [createAndStartRecognition, isTauri, startNativeRecognition],
  );

  const stopListening = useCallback(() => {
    isListeningRef.current = false;
    if (recognitionRef.current) {
      recognitionRef.current.abort();
      recognitionRef.current = null;
    }
    if (isTauri) {
      void stopNativeRecognition();
    }
    setIsListening(false);
    setInterimTranscript('');
  }, [isTauri, stopNativeRecognition]);

  /** Consume and return accumulated final transcript, then reset */
  const consumeTranscript = useCallback(() => {
    const text = accumulatedTranscriptRef.current;
    accumulatedTranscriptRef.current = '';
    return text;
  }, []);

  return {
    isListening,
    interimTranscript,
    error,
    startListening,
    stopListening,
    isSupported,
    consumeTranscript,
  };
}