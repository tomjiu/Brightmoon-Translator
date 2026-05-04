import { useState, useRef, useCallback, useEffect } from "react";

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
  zh: "zh-CN",
  en: "en-US",
  ja: "ja-JP",
  ko: "ko-KR",
  fr: "fr-FR",
  de: "de-DE",
  es: "es-ES",
  ru: "ru-RU",
  pt: "pt-BR",
  it: "it-IT",
  ar: "ar-SA",
  th: "th-TH",
  vi: "vi-VN",
  auto: "en-US",
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
    SpeechRecognition: new () => SpeechRecognitionInstance;
    webkitSpeechRecognition: new () => SpeechRecognitionInstance;
  }
}

// Check support once
const getSpeechRecognitionAPI = () => {
  if (typeof window === "undefined") return null;
  return window.SpeechRecognition || window.webkitSpeechRecognition || null;
};

export function useSpeechRecognition(): SpeechRecognitionHook {
  const [isListening, setIsListening] = useState(false);
  const [interimTranscript, setInterimTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);

  // Use ref for accumulated transcript to avoid state timing issues
  const accumulatedTranscriptRef = useRef("");
  const recognitionRef = useRef<SpeechRecognitionInstance | null>(null);
  const isListeningRef = useRef(false);
  const langRef = useRef("en-US");

  const isSupported = !!getSpeechRecognitionAPI();

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
      let interim = "";

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
      console.error("Speech recognition error:", event.error);
      if (event.error === "not-allowed") {
        setError("麦克风访问被拒绝，请允许麦克风权限");
        isListeningRef.current = false;
      } else if (event.error === "no-speech") {
        // No speech detected, continue
      } else if (event.error === "network") {
        setError("网络错误，请检查网络连接");
      } else if (event.error === "aborted") {
        // Intentional abort, ignore
      } else {
        setError(`语音识别错误: ${event.error}`);
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
        setInterimTranscript("");
      }
    };

    try {
      recognition.start();
      return recognition;
    } catch (err) {
      console.error("Failed to start speech recognition:", err);
      setError("启动语音识别失败");
      return null;
    }
  }, []);

  const startListening = useCallback(
    (lang: string = "auto") => {
      // Stop any existing recognition
      if (recognitionRef.current) {
        isListeningRef.current = false;
        recognitionRef.current.abort();
        recognitionRef.current = null;
      }

      setError(null);
      setInterimTranscript("");
      accumulatedTranscriptRef.current = "";
      langRef.current = LANG_TO_BCP47[lang] || LANG_TO_BCP47["en"];
      isListeningRef.current = true;

      const recognition = createAndStartRecognition();
      if (recognition) {
        recognitionRef.current = recognition;
      } else {
        isListeningRef.current = false;
      }
    },
    [createAndStartRecognition]
  );

  const stopListening = useCallback(() => {
    isListeningRef.current = false;
    if (recognitionRef.current) {
      recognitionRef.current.abort();
      recognitionRef.current = null;
    }
    setIsListening(false);
    setInterimTranscript("");
  }, []);

  /** Consume and return accumulated final transcript, then reset */
  const consumeTranscript = useCallback(() => {
    const text = accumulatedTranscriptRef.current;
    accumulatedTranscriptRef.current = "";
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
