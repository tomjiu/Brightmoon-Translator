# Moon Translator API 文档

本文档描述 Moon Translator 的所有 API 接口，包括 Tauri Commands、HTTP API 和事件系统。

---

## 目录

- [Tauri Commands](#tauri-commands)
  - [翻译相关](#翻译相关)
  - [窗口管理](#窗口管理)
  - [OCR 截图](#ocr-截图)
  - [配置管理](#配置管理)
  - [历史记录](#历史记录)
  - [缓存管理](#缓存管理)
  - [Hook 监控](#hook-监控)
  - [术语表](#术语表)
  - [词汇本](#词汇本)
  - [TTS](#tts)
  - [批量翻译](#批量翻译)
  - [文档翻译](#文档翻译)
  - [图片翻译](#图片翻译)
  - [质量评估](#质量评估)
- [HTTP API](#http-api)
- [事件系统](#事件系统)
- [配置格式](#配置格式)

---

## Tauri Commands

Tauri Commands 是前端通过 `invoke()` 调用的后端函数。

### 调用方式

```typescript
import { invoke } from "@tauri-apps/api/core";

// 基本调用
const result = await invoke("command_name", { param1: "value1" });

// 使用封装的 safeInvoke
import { safeInvoke } from "./services/invoke";
const [data, error] = await safeInvoke("command_name", { param1: "value1" });
```

---

### 翻译相关

#### `translate`

执行翻译请求，返回多引擎结果。

**参数**:
```typescript
interface TranslateRequest {
  text: string;    // 源文本
  from: string;    // 源语言 (如 "auto", "en", "zh")
  to: string;      // 目标语言 (如 "zh", "en", "ja")
}
```

**返回**:
```typescript
interface TranslateResponse {
  results: TranslationResult[];
  detectedLanguage: string | null;
}

interface TranslationResult {
  engine: string;      // 引擎名称
  text: string;        // 翻译结果
  latencyMs?: number;  // 延迟 (毫秒)
}
```

**示例**:
```typescript
const response = await invoke("translate", {
  request: { text: "Hello", from: "en", to: "zh" }
});
// { results: [{ engine: "Google", text: "你好", latencyMs: 120 }], detectedLanguage: "en" }
```

---

#### `translate_stream`

流式翻译，逐块返回结果。

**参数**: 同 `translate`

**返回**: `string` - 完整翻译文本

**事件**: 监听 `stream-chunk` 事件获取逐块结果

```typescript
const unlisten = await listen("stream-chunk", (event) => {
  const { chunk, done } = event.payload;
  if (done) {
    // 翻译完成
  } else {
    // 追加 chunk
  }
});

await invoke("translate_stream", {
  request: { text: "Hello World", from: "en", to: "zh" }
});
```

---

#### `translate_embedded`

行间翻译，逐行翻译并保留行号。

**参数**:
```typescript
{
  text: string;    // 多行文本
  from: string;
  to: string;
}
```

**返回**:
```typescript
interface EmbeddedLine {
  lineNumber: number;
  original: string;
  translated: string;
}
```

---

#### `translate_selection_with_text`

翻译选中文本并显示悬浮窗。

**参数**: `text: string`

**返回**: `void`

**事件**: 触发 `selection-translated` 事件

---

#### `replace_translate`

获取选中文本，翻译后替换原选区。

**参数**: 无

**返回**:
```typescript
interface ReplacementResult {
  original: string;
  replacement: string;
  success: boolean;
  error: string | null;
  fallbackToOverlay: boolean;
}
```

---

#### `back_translate`

回译 (将译文翻译回原文语言)。

**参数**:
```typescript
{
  text: string;
  from: string;
  to: string;
}
```

**返回**: `string`

---

#### `polish_translation`

润色翻译结果。

**参数**:
```typescript
{
  sourceText: string;
  translatedText: string;
  fromLang: string;
  toLang: string;
}
```

**返回**: `string` - 润色后的译文

---

#### `compare_translate`

多引擎并行对比翻译。

**参数**: 同 `translate`

**返回**: `TranslateResponse`

---

#### `detect_language`

检测文本语言。

**参数**: `text: string`

**返回**:
```typescript
interface DetectionResult {
  language: string;
  confidence: number;
}
```

---

#### `lookup_dictionary`

查词典。

**参数**: `text: string`

**返回**:
```typescript
interface DictionaryResult {
  word: string;
  phonetic: string;
  meanings: string[];
}
```

---

#### `start_clipboard_monitor` / `stop_clipboard_monitor`

启动/停止主窗口剪贴板监听（Windows：`AddClipboardFormatListener` 事件驱动 + 短 settle + 去重；非 Windows 返回错误）。

**参数**: 无

**返回**: `void`

**事件**: 后端在文本变化时发出 `clipboard-changed`（payload: `string` 剪贴板文本）。不再使用轮询 `read-clipboard` stub。

---

### 窗口管理

#### `create_overlay`

创建翻译悬浮窗。

**参数**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
  source?: string;
}
```

**返回**: `void`

---

#### `close_overlay`

关闭悬浮窗。

**参数**: 无

**返回**: `void`

---

#### `update_overlay`

更新悬浮窗内容和位置。

**参数**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
  source?: string;
  showControls?: boolean;
}
```

**返回**: `void`

---

#### `update_overlay_content`

仅更新悬浮窗内容。

**参数**:
```typescript
{
  text: string;
  source?: string;
}
```

**返回**: `void`

---

#### `update_overlay_position`

仅更新悬浮窗位置。

**参数**:
```typescript
{
  x: number;
  y: number;
}
```

**返回**: `void`

---

#### `pin_overlay` / `unpin_overlay`

固定/取消固定悬浮窗。

**参数**: 无

**返回**: `void`

---

#### `set_overlay_click_through`

设置悬浮窗鼠标穿透。

**参数**: `enabled: boolean`

**返回**: `void`

---

#### `set_overlay_follow_mode`

设置悬浮窗跟随模式。

**参数**: `mode: "none" | "cursor" | "target_bounds"`

**返回**: `void`

---

#### `refresh_overlay_position`

刷新悬浮窗位置。

**参数**: 无

**返回**: `void`

---

#### `stop_overlay_follow`

停止悬浮窗跟随。

**参数**: 无

**返回**: `void`

---

#### `hide_main_window` / `show_main_window`

隐藏/显示主窗口。

**参数**: 无

**返回**: `void`

---

#### `toggle_always_on_top`

切换窗口置顶。

**参数**: 无

**返回**: `boolean` - 是否置顶

---

#### `get_always_on_top`

获取窗口置顶状态。

**参数**: 无

**返回**: `boolean`

---

#### `get_cursor_position`

获取鼠标光标位置。

**参数**: 无

**返回**: `[number, number]` - [x, y]

---

#### `move_window_to_cursor`

移动窗口到光标位置。

**参数**: 无

**返回**: `void`

---

#### `detect_foreground_app`

检测前台应用信息。

**参数**: 无

**返回**:
```typescript
{
  name: string;
  hwnd: number;
  title: string;
}
```

---

#### `get_selected_text`

获取选中文本。

**参数**: 无

**返回**: `string`

---

#### `translate_selection`

翻译选中文本。

**参数**: 无

**返回**: `void`

---

#### `trigger_selection_translate`

触发划词翻译。

**参数**: 无

**返回**: `void`

---

#### `create_ocr_screenshot_selector` / `close_ocr_screenshot_selector`

创建/关闭 OCR 截图选择器。

**参数**: 无

**返回**: `void`

---

#### `create_ocr_region_frame` / `close_ocr_region_frame` / `hide_ocr_region_frame` / `show_ocr_region_frame`

管理 OCR 区域框。

**参数**: 无

**返回**: `void`

---

### OCR 截图

#### `capture_screen`

截取屏幕指定区域。

**参数**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
}
```

**返回**: `string` - Base64 编码的图片

---

#### `capture_full_screen`

截取全屏。

**参数**: 无

**返回**: `string` - Base64 编码的图片

---

#### `system_ocr`

系统 OCR 识别。

**参数**: `image: string` (Base64)

**返回**: `string` - 识别的文本

---

#### `system_ocr_detailed`

系统 OCR 详细识别 (包含位置信息)。

**参数**: `image: string` (Base64)

**返回**:
```typescript
interface OcrResult {
  text: string;
  lines: OcrLine[];
}

interface OcrLine {
  text: string;
  boundingBox: { x: number; y: number; width: number; height: number };
  words: OcrWord[];
}
```

---

#### `youdao_ocr`

有道 OCR 识别。

**参数**: `image: string` (Base64)

**返回**: `string`

---

#### `prepare_screenshot_snapshot`

准备截图快照。

**参数**: 无

**返回**: `string` - 快照 ID

---

#### `load_screenshot_snapshot`

加载截图快照。

**参数**: `snapshotId: string`

**返回**: `string` - Base64 图片

---

#### `crop_screenshot_snapshot`

裁剪截图快照。

**参数**:
```typescript
{
  snapshotId: string;
  x: number;
  y: number;
  width: number;
  height: number;
}
```

**返回**: `string` - Base64 图片

---

#### `capture_screenshot_region`

截取指定区域。

**参数**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
}
```

**返回**: `string` - Base64 图片

---

#### `detect_foreground_hwnd`

检测前台窗口句柄。

**参数**: 无

**返回**: `number`

---

#### `get_window_rect_cmd`

获取窗口矩形。

**参数**: `hwnd: number`

**返回**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
}
```

---

#### `get_window_title_cmd`

获取窗口标题。

**参数**: `hwnd: number`

**返回**: `string`

---

#### `detect_text_regions`

检测文本区域。

**参数**: `image: string` (Base64)

**返回**:
```typescript
interface TextRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
}
```

---

### 配置管理

#### `get_config`

获取当前配置。

**参数**: 无

**返回**: `AppConfig`

---

#### `get_default_config`

获取默认配置。

**参数**: 无

**返回**: `AppConfig`

---

#### `save_config`

保存配置。

**参数**: `config: AppConfig`

**返回**: `void`

---

#### `save_window_position`

保存窗口位置。

**参数**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
}
```

**返回**: `void`

---

#### `get_window_position`

获取窗口位置。

**参数**: 无

**返回**:
```typescript
{
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
}
```

---

#### `get_api_server_status`

获取 API 服务器状态。

**参数**: 无

**返回**:
```typescript
{
  enabled: boolean;
  port: number;
  running: boolean;
}
```

---

#### `export_config_json`

导出配置为 JSON。

**参数**: 无

**返回**: `string`

---

#### `import_config_json`

导入配置 JSON。

**参数**: `json: string`

**返回**: `void`

---

#### `get_translation_blacklist` / `update_translation_blacklist`

获取/更新翻译黑名单。

**参数**: `words: string[]` (仅 update)

**返回**: `string[]` (仅 get) / `void` (仅 update)

---

### 历史记录

#### `get_history`

获取历史记录。

**参数**:
```typescript
{
  page?: number;
  pageSize?: number;
  search?: string;
}
```

**返回**:
```typescript
interface HistoryItem {
  id: string;
  sourceText: string;
  translatedText: string;
  from: string;
  to: string;
  engine: string;
  timestamp: number;
}
```

---

#### `clear_history`

清空历史记录。

**参数**: 无

**返回**: `void`

---

#### `delete_history_item`

删除单条历史记录。

**参数**: `id: string`

**返回**: `void`

---

#### `batch_delete_history`

批量删除历史记录。

**参数**: `ids: string[]`

**返回**: `void`

---

### 缓存管理

#### `clear_cache`

清空翻译缓存。

**参数**: 无

**返回**: `void`

---

#### `cache_size`

获取缓存大小。

**参数**: 无

**返回**: `number`

---

#### `get_cache_stats`

获取缓存统计。

**参数**: 无

**返回**:
```typescript
interface CacheStats {
  hits: number;
  misses: number;
  size: number;
  hitRate: number;
}
```

---

### Hook 监控

#### `hook_inject`

注入 Hook 到目标进程。

**参数**:
```typescript
{
  hwnd: number;
  dllPath: string;
}
```

**返回**: `void`

---

#### `hook_eject`

从目标进程卸载 Hook。

**参数**: `hwnd: number`

**返回**: `void`

---

#### `hook_status`

获取 Hook 状态。

**参数**: `hwnd: number`

**返回**:
```typescript
{
  injected: boolean;
  pid: number;
}
```

---

#### `hook_read_messages`

读取 Hook 消息。

**参数**: `hwnd: number`

**返回**: `string[]`

---

#### `start_hook_monitor` / `stop_hook_monitor`

启动/停止 Hook 监控。

**参数**: 无

**返回**: `void`

---

#### `get_hook_monitor_status`

获取 Hook 监控状态。

**参数**: 无

**返回**:
```typescript
{
  running: boolean;
  targetHwnd: number | null;
}
```

---

#### `get_foreground_window_rect`

获取前台窗口矩形。

**参数**: 无

**返回**:
```typescript
{
  x: number;
  y: number;
  width: number;
  height: number;
}
```

---

### 术语表

#### `get_glossary`

获取指定语言对的术语表。

**参数**: `langPair: string` (如 "en-zh")

**返回**:
```typescript
interface GlossaryEntry {
  source: string;
  target: string;
  context?: string;
}
```

---

#### `get_all_glossary`

获取所有术语表。

**参数**: 无

**返回**: `Record<string, GlossaryEntry[]>`

---

#### `add_glossary_entry`

添加术语条目。

**参数**:
```typescript
{
  langPair: string;
  source: string;
  target: string;
  context?: string;
}
```

**返回**: `void`

---

#### `remove_glossary_entry`

删除术语条目。

**参数**:
```typescript
{
  langPair: string;
  source: string;
}
```

**返回**: `void`

---

### 词汇本

#### `get_wordbook`

获取词汇本。

**参数**:
```typescript
{
  page?: number;
  pageSize?: number;
  search?: string;
}
```

**返回**:
```typescript
interface WordbookEntry {
  id: string;
  word: string;
  translation: string;
  context?: string;
  note?: string;
  createdAt: number;
}
```

---

#### `add_wordbook_entry`

添加词汇条目。

**参数**:
```typescript
{
  word: string;
  translation: string;
  context?: string;
}
```

**返回**: `void`

---

#### `update_wordbook_note`

更新词汇笔记。

**参数**:
```typescript
{
  id: string;
  note: string;
}
```

**返回**: `void`

---

#### `delete_wordbook_entry`

删除词汇条目。

**参数**: `id: string`

**返回**: `void`

---

#### `batch_delete_wordbook`

批量删除词汇。

**参数**: `ids: string[]`

**返回**: `void`

---

#### `clear_wordbook`

清空词汇本。

**参数**: 无

**返回**: `void`

---

#### `search_wordbook`

搜索词汇。

**参数**: `query: string`

**返回**: `WordbookEntry[]`

---

#### `export_wordbook_csv`

导出词汇本为 CSV。

**参数**: 无

**返回**: `string` - CSV 内容

---

### TTS

#### `text_to_speech`

文本转语音。

**参数**:
```typescript
{
  text: string;
  lang?: string;
  voice?: string;
}
```

**返回**: `string` - Base64 音频

---

#### `get_tts_voices`

获取可用语音列表。

**参数**: 无

**返回**:
```typescript
interface TtsVoice {
  name: string;
  lang: string;
  gender: string;
}
```

---

### 批量翻译

#### `batch_submit`

提交批量翻译任务。

**参数**:
```typescript
{
  texts: string[];
  from: string;
  to: string;
  concurrency?: number;
}
```

**返回**: `string` - 任务 ID

---

#### `batch_cancel`

取消批量任务。

**参数**: `taskId: string`

**返回**: `void`

---

#### `batch_pause` / `batch_resume`

暂停/恢复批量任务。

**参数**: `taskId: string`

**返回**: `void`

---

#### `batch_retry_failed`

重试失败的项。

**参数**: `taskId: string`

**返回**: `void`

---

#### `batch_get_progress`

获取批量任务进度。

**参数**: `taskId: string`

**返回**:
```typescript
interface BatchProgress {
  total: number;
  completed: number;
  failed: number;
  running: number;
}
```

---

#### `batch_get_results`

获取批量任务结果。

**参数**: `taskId: string`

**返回**:
```typescript
interface BatchResult {
  index: number;
  original: string;
  translated: string;
  success: boolean;
  error?: string;
}
```

---

#### `batch_get_status`

获取批量任务状态。

**参数**: `taskId: string`

**返回**: `"pending" | "running" | "paused" | "completed" | "cancelled"`

---

#### `batch_reset`

重置批量任务。

**参数**: `taskId: string`

**返回**: `void`

---

#### `tm_export` / `tm_import`

导出/导入翻译记忆。

**参数**: `content: string` (仅 import)

**返回**: `string` (仅 export) / `void` (仅 import)

---

#### `tm_get_stats`

获取翻译记忆统计。

**参数**: 无

**返回**:
```typescript
interface TmStats {
  totalEntries: number;
  languagePairs: number;
}
```

---

#### `tm_search`

搜索翻译记忆。

**参数**:
```typescript
{
  query: string;
  from?: string;
  to?: string;
  threshold?: number;
}
```

**返回**:
```typescript
interface TmMatch {
  source: string;
  target: string;
  similarity: number;
}
```

---

### 文档翻译

#### `open_pdf` / `translate_pdf`

打开/翻译 PDF 文件。

**参数**: `path: string`

**返回**: 页面数据

---

#### `open_docx` / `translate_docx` / `translate_docx_preview`

DOCX（已注册 IPC + Documents「Word」页）。

| 命令 | 参数 | 返回 |
|------|------|------|
| `open_docx` | `filePath` | `DocxDocument` |
| `translate_docx_preview` | `inputPath`, `fromLang`, `toLang` | `TranslatedDocx`（内存预览） |
| `translate_docx` | `inputPath`, `outputPath`, `fromLang`, `toLang` | 写出结果 + `docx-progress` 事件 |

---

#### `open_excel` / `translate_excel` / `translate_excel_preview`

Excel（已注册 + Documents「Excel」页）。参数同 DOCX 风格：`filePath` / `inputPath`+`outputPath`。

---

#### `open_pptx` / `translate_pptx` / `translate_pptx_preview`

PPTX（已注册 + Documents「PPT」页）。参数同 DOCX 风格。

---

#### `open_epub` / `translate_epub`

EPUB 文件操作。

**参数**: `path: string`

**返回**: 电子书数据

---

#### `open_subtitle` / `translate_subtitle` / `export_subtitle_file` / `translate_subtitle_text`

字幕文件操作。

| 命令 | 参数 | 返回 |
|------|------|------|
| `open_subtitle` | `filePath` | `SubtitleDocument` |
| `translate_subtitle` | `filePath`, `fromLang`, `toLang` | `TranslatedSubtitle`（含译文） |
| `export_subtitle_file` | **`entries`**（内存条目，须含 `translatedText`）, `format`, `outputPath`, `bilingual` | 写出路径 |
| `translate_subtitle_text` | `text`, `fromLang`, `toLang` | `string` |

**注意:** 导出必须传翻译后的 `entries`，不要再传源文件路径重读（会丢掉译文）。

---

### 图片翻译

#### `translate_image` / `preview_image_translation` / `translate_image_base64`

图片文件翻译（已注册 IPC + Documents「图片」页）。

| 命令 | 参数 | 返回 |
|------|------|------|
| `translate_image` | `inputPath`, `outputPath`, `fromLang`, `toLang`, 可选 OCR | `ImageTranslationResult`（含 `outputPath`） |
| `preview_image_translation` | `inputPath`, `lang`, 可选 OCR | `ImagePreview`（行+框） |
| `translate_image_base64` | `base64Data`, `fromLang`, `toLang` | base64 PNG + 统计 |

---

### 质量评估

#### `score_translation`

评估翻译质量。

**参数**:
```typescript
{
  source: string;
  translation: string;
  from: string;
  to: string;
}
```

**返回**:
```typescript
interface QualityScore {
  overall: number;
  fluency: number;
  adequacy: number;
  details: string;
}
```

---

#### `compare_engine_quality`

对比引擎翻译质量。

**参数**:
```typescript
{
  text: string;
  from: string;
  to: string;
}
```

**返回**:
```typescript
interface EngineComparison {
  engine: string;
  translation: string;
  score: QualityScore;
}
```

---

## HTTP API

当配置 `apiServerEnabled: true` 时，应用会启动 HTTP API 服务器（仅 `127.0.0.1`）。

**默认地址**: `http://127.0.0.1:60828`

### 鉴权（S1）

| 路径 | 鉴权 |
|------|------|
| `GET /health` | **否**（探活） |
| 其余全部 | **是** |

请求头二选一：

```http
Authorization: Bearer <apiServerToken>
X-Api-Token: <apiServerToken>
```

- 配置字段：`apiServerToken`（高级设置可查看/复制/重新生成）。
- 首次启动 API 且令牌为空时，服务端会 **自动生成 UUID** 并写入配置。
- 浏览器扩展：popup 中填写同一令牌 → `chrome.storage.local.desktopApiToken`。
- 错误：`401 Unauthorized`；令牌未配置：`503`。

### 控制路由（需鉴权，触发桌面 UI / 热键等价动作）

> 需先启用 `apiServerEnabled`（高级设置 → 浏览器扩展），并携带 Bearer / `X-Api-Token`。实现：`src-tauri/src/api_server.rs`。

| 方法 | 路径 | 作用 | 内部事件 |
|------|------|------|----------|
| POST | `/control/show` | 显示并聚焦主窗口 | `window.show` + `set_focus` |
| POST | `/control/selection_translate` | 划词翻译 | `trigger-translate-selection` |
| POST | `/control/ocr_translate` | OCR 截图翻译 | `trigger-ocr-screenshot` |
| POST | `/control/open_settings` | 打开设置页 | `navigate` → `settings` |

**响应**: `{ "ok": true }`（无 handle 时 `503`）

**示例**:

```bash
curl -X POST http://127.0.0.1:60828/control/show \
  -H "Authorization: Bearer <apiServerToken>"

curl -X POST http://127.0.0.1:60828/control/selection_translate \
  -H "X-Api-Token: <apiServerToken>"
```

浏览器扩展桥接失败时，请在桌面高级设置启用 API 服务器并配置同一令牌（见 `extension/README.md`）。

### 端点列表

#### `GET /health`

健康检查（**无需令牌**）。

**响应**:
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

#### `POST /translate`

翻译文本（需鉴权）。

**请求**:
```json
{
  "text": "Hello",
  "from": "en",
  "to": "zh",
  "stream": false
}
```

**响应**:
```json
{
  "results": [
    {
      "engine": "Google",
      "text": "你好"
    }
  ],
  "detectedLanguage": "en"
}
```

---

#### `POST /translate/primary`

使用主引擎翻译。

**请求**: 同 `/translate`

**响应**:
```json
{
  "engine": "primary",
  "text": "你好"
}
```

---

#### `GET /config`

获取配置 (密钥脱敏)。

**响应**: AppConfig JSON

---

#### `POST /config`

更新配置 (部分更新)。

**请求**:
```json
{
  "default_to": "en"
}
```

**响应**: 更新后的 AppConfig

---

#### `GET /history`

获取历史记录。

**响应**: HistoryItem 数组

---

#### `GET /engines`

获取可用引擎列表。

**响应**:
```json
{
  "engines": ["LLM", "Google", "Youdao"],
  "count": 3
}
```

---

#### `POST /browser/translate`

浏览器扩展翻译接口。

**请求**:
```json
{
  "text": "Hello",
  "sourceLang": "en",
  "targetLang": "zh",
  "mode": "translate"
}
```

**响应**:
```json
{
  "translated": "你好",
  "engine": "Google"
}
```

---

#### `GET /glossary`

获取术语表。

**响应**: GlossaryEntry 数组

---

#### `POST /glossary`

添加术语条目。

**请求**:
```json
{
  "langPair": "en-zh",
  "source": "Hello",
  "target": "你好"
}
```

---

#### `DELETE /glossary`

删除术语条目。

**请求**:
```json
{
  "langPair": "en-zh",
  "source": "Hello"
}
```

---

#### `GET /blacklist`

获取翻译黑名单。

**响应**:
```json
{
  "words": ["word1", "word2"]
}
```

---

#### `POST /blacklist`

更新翻译黑名单。

**请求**:
```json
{
  "words": ["word1", "word2", "word3"]
}
```

---

#### `GET /cache/stats`

获取缓存统计。

**响应**:
```json
{
  "hits": 100,
  "misses": 20,
  "size": 50,
  "hitRate": 0.83
}
```

---

#### `POST /cache/clear`

清空缓存。

**响应**: `200 OK`

---

### CORS 配置

API 服务器允许以下来源访问:
- `chrome-extension://*`
- `moz-extension://*`
- `http://localhost:*`
- `http://127.0.0.1:*`

---

## 事件系统

Tauri 事件用于前后端实时通信。

### 前端监听事件

```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<string>("event-name", (event) => {
  console.log(event.payload);
});

// 清理监听
unlisten();
```

### 事件列表

| 事件名 | 方向 | 载荷 | 说明 |
|--------|------|------|------|
| `stream-chunk` | 后端→前端 | `{ chunk: string, done: boolean }` | 流式翻译块 |
| `trigger-ocr-screenshot` | 后端→前端 | `()` | 触发 OCR 截图 |
| `selection-translated` | 后端→前端 | `{ source, translated, engine }` | 划词翻译结果 |
| `auto-copy` | 后端→前端 | `string` | 自动复制文本 |
| `navigate` | 后端→前端 | `string` | 页面导航 |
| `clipboard-changed` | 后端→前端 | `string` | 主剪贴板监听：新文本（事件驱动） |
| `trigger-translate-selection` | 后端→前端 | `()` | 触发划词翻译 |
| `trigger-replace-translate` | 后端→前端 | `()` | 触发替换翻译 |

---

## 配置格式

配置文件位置: `%APPDATA%/moontranslator/config.json`

### 完整配置结构

```json
{
  "llm": {
    "provider": "deepseek",
    "apiKey": "sk-xxx",
    "apiKeys": ["sk-xxx", "sk-yyy"],
    "baseUrl": "https://api.deepseek.com/v1",
    "model": "deepseek-chat"
  },
  "engines": {
    "google": {
      "enabled": true
    },
    "baidu": {
      "enabled": false,
      "appId": "",
      "secret": ""
    },
    "youdao": {
      "enabled": true,
      "useAi": false,
      "ocrAppKey": "3d9fa94028675971",
      "ocrAppSecret": "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p"
    },
    "deepl": {
      "enabled": false,
      "apiKey": "",
      "pro": false
    },
    "deeplx": {
      "enabled": false,
      "apiKey": null,
      "pro": false
    },
    "microsoft": {
      "enabled": false
    },
    "yandex": {
      "enabled": false
    }
  },
  "defaultFrom": "auto",
  "defaultTo": "zh",
  "customPrompt": "",
  "promptTemplates": [
    {
      "name": "技术文档",
      "prompt": "请以专业的技术文档风格翻译..."
    }
  ],
  "clipboardMonitor": false,
  "autoCopyResult": false,
  "autoCopyMode": "translated",
  "translationMask": false,
  "apiServerEnabled": false,
  "apiServerPort": 60828,
  "hotkeys": {
    "ocrTranslate": "Ctrl+Shift+T",
    "showWindow": "Ctrl+T",
    "translateSelection": "Ctrl+Shift+Y",
    "replaceTranslate": "Ctrl+Shift+R",
    "toggleOverlayClickThrough": "Ctrl+Shift+Escape"
  },
  "proxy": {
    "enabled": false,
    "proxyType": "http",
    "host": "",
    "port": 7890,
    "username": "",
    "password": ""
  },
  "windowX": null,
  "windowY": null,
  "windowWidth": null,
  "windowHeight": null,
  "windowFollowMode": "none",
  "translationBlacklist": [],
  "routingStrategy": "fallback_on_error",
  "overlayLevel": 2,
  "overlayAutoDismissMs": 3000,
  "overlayFollowMode": "none",
  "ocrInterval": 2000,
  "ocrClickThrough": false,
  "ocrAutoBindWindow": true,
  "hook": {
    "enabledSources": ["uia", "clipboard", "ocr", "hook"],
    "showOverlay": true,
    "autoCopy": false,
    "enabled": true,
    "uiaIntervalMs": 500,
    "ocrIntervalMs": 5000
  },
  "tmEnabled": false,
  "tmThreshold": 0.8,
  "furiganaEnabled": false,
  "ttsAutoPlay": false,
  "ttsVoice": "",
  "ttsClientToken": "6A5AA1D4EAFF4E9FB37E23D68491D6F4",
  "requestTimeoutSecs": 30,
  "ocrRequestTimeoutSecs": 30,
  "youdaoCdnTimeoutSecs": 10,
  "youdaoCdnDownloadTimeoutSecs": 60,
  "ocrCacheTtlSecs": 300,
  "realtimeTranslate": true,
  "realtimeDelayMs": 500
}
```

### 配置项说明

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `llm.provider` | string | "deepseek" | LLM 提供商 |
| `llm.apiKey` | string | "" | API 密钥 |
| `llm.apiKeys` | string[] | [] | 多个 API 密钥 (轮询) |
| `llm.baseUrl` | string | "https://api.deepseek.com/v1" | API 基础 URL |
| `llm.model` | string | "deepseek-chat" | 模型名称 |
| `defaultFrom` | string | "auto" | 默认源语言 |
| `defaultTo` | string | "zh" | 默认目标语言 |
| `routingStrategy` | string | "fallback_on_error" | 路由策略 |
| `apiServerEnabled` | boolean | false | 启用 HTTP API |
| `apiServerPort` | number | 60828 | API 端口 |
| `overlayLevel` | number | 2 | 悬浮窗层级 |
| `tmEnabled` | boolean | false | 启用翻译记忆 |
| `tmThreshold` | number | 0.8 | 翻译记忆匹配阈值 |
| `requestTimeoutSecs` | number | 30 | 请求超时 (秒) |
| `realtimeTranslate` | boolean | true | 实时翻译 |
| `realtimeDelayMs` | number | 500 | 实时翻译防抖 (毫秒) |

### 路由策略

| 策略 | 说明 |
|------|------|
| `primary_only` | 仅使用主引擎 |
| `fallback_on_error` | 失败时降级到下一个引擎 |
| `parallel_compare` | 并行调用所有引擎，返回所有结果 |
| `cost_aware` | 免费引擎优先 |
| `latency_first` | 延迟最低的引擎优先 |

### 语言代码

| 代码 | 语言 |
|------|------|
| `auto` | 自动检测 |
| `zh` | 中文 |
| `en` | 英语 |
| `ja` | 日语 |
| `ko` | 韩语 |
| `fr` | 法语 |
| `de` | 德语 |
| `es` | 西班牙语 |
| `ru` | 俄语 |
| `pt` | 葡萄牙语 |
| `it` | 意大利语 |
| `ar` | 阿拉伯语 |
| `th` | 泰语 |
| `vi` | 越南语 |
