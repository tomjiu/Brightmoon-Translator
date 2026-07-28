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
interface LlmProviderEntry {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  priority: number;
  enabled: boolean;
  models: string[];
}

interface LlmConfig {
  provider: string;
  apiKey: string;
  apiKeys: string[];
  baseUrl: string;
  model: string;
  providers: LlmProviderEntry[];
}

interface EnginesConfig {
  google: { enabled: boolean };
  baidu: { enabled: boolean; appId: string; secret: string };
  youdao: {
    enabled: boolean;
    useAi: boolean;
    ocrAppKey?: string;
    ocrAppSecret?: string;
  };
  deepl: { enabled: boolean; apiKey: string; pro: boolean };
  deeplx: { enabled: boolean; apiKey?: string; pro: boolean };
  caiyun?: { enabled: boolean; apiToken: string };
  microsoft: { enabled: boolean };
  yandex: { enabled: boolean };
  offline: {
    enabled: boolean;
    autoSwitch: boolean;
    downloadedModels: string[];
    modelDir: string;
  };
  /** Example sentences (not MT) */
  tatoeba?: { enabled: boolean };
  baiduWeb?: { enabled: boolean };
  caiyunWeb?: { enabled: boolean };
  volcengineWeb?: { enabled: boolean };
  transmart?: { enabled: boolean };
  papago?: { enabled: boolean };
}

interface HotkeyConfig {
  ocrTranslate: string;
  showWindow: string;
  translateSelection: string;
  replaceTranslate: string;
  toggleOverlayClickThrough: string;
}

/** Desktop 划词 / 取词 (Youdao-inspired) */
export type SelectionTriggerMode = 'hotkey_only' | 'auto_on_select' | 'pop_button';

export interface SelectionUxConfig {
  /** Select → popup: hotkey only vs auto after mouse-up */
  triggerMode: SelectionTriggerMode;
  /** Mouse dwell → dictionary popup (system-wide; phased) */
  hoverDictionary: boolean;
  hoverDwellMs: number;
  /** Empty UIA/clipboard → OCR near cursor (phased) */
  ocrForcePickup: boolean;
  /** Min selection length for auto-on-select */
  autoMinChars: number;
  /** Min drag pixels before auto-on-select (Easydict-style) */
  minDragPx?: number;
  /** Process names to skip (no .exe), e.g. potplayer */
  excludeProcesses?: string[];
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

export type AutoCopyMode = 'translated' | 'source' | 'both' | 'none';
export type RoutingStrategy =
  | 'primary_only'
  | 'fallback_on_error'
  | 'parallel_compare'
  | 'cost_aware'
  | 'latency_first';
export type OcrEngine = 'auto' | 'winrt' | 'youdao' | 'tesseract' | 'rapid' | 'paddle';
type WindowFollowMode = 'none' | 'cursor';

export interface SyncConfig {
  enabled: boolean;
  serverUrl: string;
  username: string;
  password: string;
  remoteDir: string;
  intervalMins: number;
  syncConfig: boolean;
  syncGlossary: boolean;
  syncHistory: boolean;
  syncWordbook: boolean;
  lastSyncAt: number;
  lastSyncStatus: string;
}

export interface SyncStatus {
  success: boolean;
  message: string;
  syncedAt: number;
  uploaded: string[];
  downloaded: string[];
}

export interface AppConfig {
  llm: LlmConfig;
  engines: EnginesConfig;
  defaultFrom: string;
  defaultTo: string;
  customPrompt: string;
  promptTemplates: PromptTemplate[];
  llmTemperature: number;
  llmMaxTokens: number;
  clipboardMonitor: boolean;
  /** Replace delivery: true = paste via clipboard; false = Unicode type. */
  useClipboardOutput: boolean;
  autoCopyResult: boolean;
  autoCopyMode: AutoCopyMode;
  translationMask: boolean;
  apiServerEnabled: boolean;
  apiServerPort: number;
  /** Local HTTP bridge token (Bearer). Required when API is enabled. */
  apiServerToken: string;
  hotkeys: HotkeyConfig;
  selectionUx?: SelectionUxConfig;
  proxy: ProxyConfig;
  windowX?: number;
  windowY?: number;
  windowWidth?: number;
  windowHeight?: number;
  windowFollowMode: WindowFollowMode;
  translationBlacklist: string[];
  routingStrategy?: RoutingStrategy | null;
  engineOrder?: string[];
  ocrEngine: OcrEngine;
  /** Rapid/Paddle sidecar path when ocrEngine is rapid|paddle */
  offlineOcr?: { backend: string; pluginDir: string };
  overlayLevel?: number;
  overlayAutoDismissMs?: number;
  overlayFollowMode?: 'none' | 'cursor' | 'target_bounds';
  ocrInterval?: number;
  ocrClickThrough?: boolean;
  ocrAutoBindWindow?: boolean;
  /** Nested hook monitor config (authoritative). Do not use ghost top-level flags. */
  hook?: HookConfig;
  tmEnabled?: boolean;
  tmThreshold?: number;
  furiganaEnabled?: boolean;
  ttsAutoPlay?: boolean;
  ttsVoice?: string;
  /** edge | openai | youdao */
  ttsProvider?: string;
  openaiTts?: {
    apiKey: string;
    baseUrl: string;
    model: string;
    voice: string;
    speed: number;
  };
  httpTimeoutSecs?: number;
  ocrTimeoutSecs?: number;
  llmTimeoutSecs?: number;
  translationTimeoutSecs?: number;
  edgeTtsToken?: string;
  sync?: SyncConfig;
  /** External vocabulary collection (Eudic / Anki / Shanbay / Youdao / Maimemo). Not FSRS. */
  collection?: CollectionConfig;
}

export interface CollectionConfig {
  eudic: {
    enabled: boolean;
    token: string;
    bookName: string;
  };
  anki: {
    enabled: boolean;
    port: number;
    deck: string;
    model: string;
  };
  shanbay: {
    enabled: boolean;
    credential: string;
    wordbookId: string;
  };
  youdao: {
    enabled: boolean;
    cookie: string;
    lan: string;
  };
  maimemo: {
    enabled: boolean;
    token: string;
    notepadId: string;
    notepadTitle: string;
  };
  autoPushOnSave: boolean;
}

export interface CollectionTargetResult {
  target: string;
  ok: boolean;
  message: string;
}

export interface CollectionPushReport {
  results: CollectionTargetResult[];
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
  { code: 'auto', name: '自动检测' },
  { code: 'zh', name: '中文' },
  { code: 'en', name: 'English' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'fr', name: 'Français' },
  { code: 'de', name: 'Deutsch' },
  { code: 'es', name: 'Español' },
  { code: 'ru', name: 'Русский' },
  { code: 'pt', name: 'Português' },
  { code: 'it', name: 'Italiano' },
  { code: 'ar', name: 'العربية' },
  { code: 'th', name: 'ไทย' },
  { code: 'vi', name: 'Tiếng Việt' },
] as const;

// Language detection types
export interface DetectionResult {
  language: string;
  confidence: number;
  name: string;
}

// OCR text region detection types
export interface TextRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  lineCount: number;
  textPreview: string;
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
  | 'snake_case'
  | 'SNAKE_CASE'
  | 'kebab-case'
  | 'camelCase'
  | 'PascalCase'
  | 'dot.notation'
  | 'Title Case';

export const VARIABLE_FORMATS: VariableFormat[] = [
  'snake_case',
  'SNAKE_CASE',
  'kebab-case',
  'camelCase',
  'PascalCase',
  'dot.notation',
  'Title Case',
];

// Batch translation types
export type BatchTaskStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
export type BatchJobStatus = 'idle' | 'running' | 'paused' | 'completed' | 'cancelled' | 'failed';

export interface BatchConfig {
  concurrency: number;
  fromLang: string;
  toLang: string;
  engine?: string;
  continueOnError: boolean;
}

export interface BatchTask {
  id: string;
  index: number;
  text: string;
  fromLang: string;
  toLang: string;
  status: BatchTaskStatus;
  result?: string;
  error?: string;
}

export interface BatchProgress {
  jobId: string;
  total: number;
  completed: number;
  failed: number;
  currentIndex?: number;
  status: BatchJobStatus;
}

// TM (Translation Memory) types
export interface TmExportEntry {
  source: string;
  target: string;
  fromLang: string;
  toLang: string;
  engine: string;
  timestamp: number;
}

export interface TmExportData {
  version: number;
  entries: TmExportEntry[];
  exportedAt: number;
}

export interface TmStats {
  total: number;
  langPairs: Array<[string, string, number]>;
}

// Translation quality scoring types
export interface TranslationScoreDetail {
  name: string;
  score: number;
  weight: number;
  description: string;
}

export interface TranslationScore {
  overall: number;
  bleuApprox: number;
  lengthRatio: number;
  terminology: number;
  fluency: number;
  details: TranslationScoreDetail[];
  timestamp: number;
}

export interface EngineScore {
  engine: string;
  score: TranslationScore;
  latencyMs: number;
  text: string;
  translated: string;
}
