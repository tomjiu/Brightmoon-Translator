// Translation types
export interface TranslationResult {
  engine: string;
  text: string;
  /** Optional latency in milliseconds (populated by LatencyFirst strategy) */
  latencyMs?: number;
}

export interface TranslateResponse {
  results: TranslationResult[];
  detectedLanguage?: string;
}

// History types
export interface HistoryItem {
  id: string;
  sourceText: string;
  translatedText: string;
  from: string;
  to: string;
  engine: string;
  timestamp: number;
}

// Config types
interface LlmConfig {
  provider: "openai" | "deepseek" | "custom";
  apiKey: string;
  apiKeys: string[];
  baseUrl: string;
  model: string;
}

interface EnginesConfig {
  google: { enabled: boolean };
  baidu: { enabled: boolean; appId: string; secret: string };
  youdao: { enabled: boolean; useAi: boolean };
  deepl: { enabled: boolean; apiKey: string; pro: boolean };
  deeplx: { enabled: boolean; apiKey?: string; pro: boolean };
  microsoft: { enabled: boolean };
  yandex: { enabled: boolean };
}

interface HotkeyConfig {
  ocrTranslate: string;
  showWindow: string;
  translateSelection: string;
  replaceTranslate?: string;
}

interface ProxyConfig {
  enabled: boolean;
  proxyType: string;
  host: string;
  port: number;
  username: string;
  password: string;
}

interface PromptTemplate {
  name: string;
  prompt: string;
}

export type AutoCopyMode = "translated" | "source" | "both" | "none";
type WindowFollowMode = "none" | "cursor";

export interface AppConfig {
  llm: LlmConfig;
  engines: EnginesConfig;
  defaultFrom: string;
  defaultTo: string;
  customPrompt: string;
  promptTemplates: PromptTemplate[];
  clipboardMonitor: boolean;
  autoCopyResult: boolean;
  autoCopyMode: AutoCopyMode;
  translationMask: boolean;
  apiServerEnabled: boolean;
  apiServerPort: number;
  hotkeys: HotkeyConfig;
  proxy: ProxyConfig;
  windowX?: number;
  windowY?: number;
  windowWidth?: number;
  windowHeight?: number;
  windowFollowMode: WindowFollowMode;
  translationBlacklist: string[];
  ocrInterval?: number;
  ocrClickThrough?: boolean;
  ocrAutoBindWindow?: boolean;
  hookShowOverlay?: boolean;
  hookAutoCopy?: boolean;
  hook?: HookConfig;
  tmEnabled?: boolean;
  tmThreshold?: number;
  furiganaEnabled?: boolean;
}

interface HookConfig {
  enabledSources?: string[];
  showOverlay?: boolean;
  autoCopy?: boolean;
  enabled?: boolean;
  uiaIntervalMs?: number;
  ocrIntervalMs?: number;
}

// Language definitions
export const LANGUAGES = [
  { code: "auto", name: "自动检测" },
  { code: "zh", name: "中文" },
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "ru", name: "Русский" },
  { code: "pt", name: "Português" },
  { code: "it", name: "Italiano" },
  { code: "ar", name: "العربية" },
  { code: "th", name: "ไทย" },
  { code: "vi", name: "Tiếng Việt" },
] as const;

// Language detection types
export interface DetectionResult {
  language: string;
  confidence: number;
  name: string;
}

// Embedded translation types
export interface EmbeddedLine {
  lineNumber: number;
  original: string;
  translated: string;
}

// Dictionary types
interface DictionaryDefinition {
  definition: string;
  example?: string;
  synonyms: string[];
  antonyms: string[];
}

interface DictionaryMeaning {
  partOfSpeech: string;
  definitions: DictionaryDefinition[];
}

export interface DictionaryResult {
  word: string;
  phonetic?: string;
  meanings: DictionaryMeaning[];
  sourceUrls: string[];
}

// Variable name format types
export type VariableFormat =
  | "snake_case"
  | "SNAKE_CASE"
  | "kebab-case"
  | "camelCase"
  | "PascalCase"
  | "dot.notation"
  | "Title Case";

export const VARIABLE_FORMATS: VariableFormat[] = [
  "snake_case",
  "SNAKE_CASE",
  "kebab-case",
  "camelCase",
  "PascalCase",
  "dot.notation",
  "Title Case",
];
