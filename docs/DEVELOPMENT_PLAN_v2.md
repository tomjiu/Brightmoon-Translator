# Development Plan v2 — 健康度修复与债务清理 (2026-07-31)

**定位**：与 [DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md) (v1) 互补。v1 是**功能扩展**路线图（浏览器双语 / PDF 排版 / OCR M2 / 捕获质量）；v2 是**健康度修复**计划，聚焦四路并行审计发现的 117 条具体问题（P0×14, P1×53, P2×50）。

**原则**：先修正确性，再清死代码，再补资源管理，最后补一致性。所有改动不破坏 v1 已冻结的 OCR session 生命周期与 Hook DLL IAT 契约。

**审计依据**：4 路并行子代理走查覆盖
- 翻译引擎管线（engine/, services/, translate_cmd, ai_cmd, batch, cache, post_process）
- 选择/覆盖层/Hook（selection/, overlay/, hook_inject, hook-dll/）
- OCR/文档/词汇（ocr_*, pdf/docx/epub, dictionary, domain, event_store）
- 前端架构（App.tsx, stores, pages, types, i18n, extension, CI）

---

## 0. 执行摘要 — 风险全景

| 风险类别 | 数量 | 最严重项 |
|---------|------|---------|
| 正确性缺陷 (P0) | 14 | Hook 共享内存 torn read、AI 命令绕 façade、WinRT OCR 阻塞 tokio、TS 类型漂移 |
| 资源泄漏 (P1) | 12 | mouse_hook/auto_watch 线程泄漏、overlay 窗口不销毁、ocr_offline 临时文件泄漏 |
| 死代码 (P1) | ~13 命令 + 3 方法 | transform_variable_name / cycle_variable_name / translate_selection_with_text / compare_translate / ai_context_translate / system_ocr / capture_full_screen / detect_text_regions / hook_read_messages / hook_process_messages / event_store 时间旅行 |
| 类型/配置漂移 (P0/P1) | 8 | LlmProviderEntry.apiFormat、YoudaoConfig、CSP null、CI actions 版本 |
| 一致性债务 (P2) | 50+ | unsafe 缺 SAFETY 注释、硬编码中文、OEM 编码未用、println 替 tracing |

**核心判断**：v1 的功能扩展（浏览器双语 / PDF / OCR M2）必须建立在 v2 的正确性修复之上。例如：
- AI 命令绕 façade → v1 Tier 1 B8（LLM 批量分隔符）无法复用 TM/cache
- Hook 共享内存 race → v1 Tier 2（Hook verdict）无法验证 IAT 正确性
- WinRT OCR 阻塞 → v1 Tier 2 O7（hot_start）收益被阻塞抵消
- TS 类型漂移 → v1 Tier 1（浏览器扩展）配置无法往返

---

## 1. Tier S0 — 正确性必修 (P0)

目标：消除数据竞争、死锁、阻塞、类型丢失。**必须在 v1 任何 Tier 之前完成**。

### S0-1 Hook 共享内存 torn read 修复
- **文件**：`src-tauri/hook-dll/src/hook_text.cpp:197-222`、`src-tauri/src/hook_inject.rs:312-373`
- **问题**：DLL 侧 write_offset + memcpy + sequence++ 非原子；宿主侧 read_messages 读 write_offset 后遍历，DLL 可并发修改，导致 torn read 或越界
- **修复**：序列号双缓冲契约
  1. DLL 侧：先写数据，再 `InterlockedIncrement(&header->sequence)`（store-release 语义）
  2. 宿主侧：先 `load(sequence)` (acquire)，遍历后再 `load(sequence)` 校验不变，否则丢弃本轮
  3. 环形 wrap 时单次 sequence++（移除 SendTextToHost 末尾的二次自增）
- **验证**：高频 hook 文本注入下，宿主侧无越界 / 无半消息

### S0-2 present.rs blocking_lock 死锁修复
- **文件**：`src-tauri/src/selection/present.rs:154-166, 386-394`
- **问题**：async 函数内 `config.blocking_lock()`（Tokio Mutex），持锁任务被抢占时阻塞 worker 线程，与同文件 `config.lock().await` 指向同一把锁，死锁风险
- **修复**：全部改为 `.lock().await`；或将配置快照在调用前传入
- **验证**：cargo check + 划词压力测试无 deadlock

### S0-3 AI 命令绕 façade 修复
- **文件**：`src-tauri/src/commands/ai_cmd.rs:53,97,131,165,201`
- **问题**：5 个 AI 命令直构 `LlmEngine::with_multiple_keys()`，跳过 TranslationService 的 TM/cache/glossary/pre-post-process；同时硬编码 `temperature:0.3 / max_tokens:4096`，忽略用户配置；强制 OpenAI wire format，Anthropic/Gemini 不可用
- **修复**：
  1. 在 `TranslationService` 暴露 `run_ai_enhanced(channel, text, AiKind)` 入口，复用 TM/cache/glossary
  2. AI 命令改为 `state.translation.service.run_ai_enhanced(...).await`
  3. 引擎构造改用 `config.llm.resolve_endpoints()` + `.with_temperature(config.llm_temperature).with_max_tokens(config.llm_max_tokens)`
- **验证**：AI 命令路径能命中缓存、能读取用户 LLM 温度、Anthropic 端点可调

### S0-4 WinRT OCR 阻塞 tokio 修复
- **文件**：`src-tauri/src/ocr_engine.rs:24-77`、`src-tauri/src/commands/capture.rs:1102-1179`
- **问题**：WinRT OCR 内部全 `.get()` 同步阻塞，async 上下文调用时阻塞 tokio worker
- **修复**：`tokio::task::spawn_blocking` 包裹整个 `run_winrt_ocr`；capture.rs 的 `system_ocr_detailed` 委托给 `ocr_engine::run_winrt_ocr` 消除重复实现
- **验证**：高频 OCR 调用下 tokio runtime 不卡顿

### S0-5 LlmProviderEntry.apiFormat 类型漂移修复
- **文件**：`src/types/index.ts:26-35` vs `src-tauri/src/models/config.rs:44`
- **问题**：Rust 序列化 `apiFormat` 字段，TS 类型缺失，往返时静默丢失，Anthropic/Gemini 配置失效
- **修复**：`LlmProviderEntry` 补 `apiFormat: 'openai' | 'anthropic' | 'gemini'`
- **关联**：S0-3 修复后 AI 命令能正确路由多提供商

### S0-6 dictionary_history 迁移脚本补齐
- **文件**：`src-tauri/migrations/003_dictionary_history.sql`（新建）、`src-tauri/src/infrastructure/event_store.rs:129-150`
- **问题**：dictionary_history 表在运行时 `init_schema()` 建表，迁移脚本缺失，依赖运行时副作用
- **修复**：新增 `003_dictionary_history.sql`，删除 `init_schema()` 中的 DDL（保留 IF NOT EXISTS 兜底）
- **关联**：同步处理 cards / card_patches / oxford_dict / gpt_dict / core_vocabulary 的双轨 schema

### S0-7 event_store 时间旅行死代码处理
- **文件**：`src-tauri/src/infrastructure/event_store.rs:231-240`
- **问题**：`get_card_at_time` / `load_events_before` 从未暴露给前端，纯死代码
- **修复**：删除这两个方法（及 `load_events_before` 的依赖）；若后续需要时间旅行功能再补 `#[tauri::command]`
- **验证**：cargo check 无未使用警告

### S0-8 ecdict.db / moon_hook.dll 路径解析收敛
- **文件**：`src-tauri/src/lib.rs:147-169`、`src-tauri/src/hook_inject.rs:423-454`
- **问题**：ecdict.db 10 个硬编码候选路径，moon_hook.dll 17 个候选路径，含开发期相对路径，release 易失效
- **修复**：收敛为「exe 同级 + tauri resources + 配置项显式路径」三段式；失败时返回候选列表供 UI 展示
- **验证**：dev/release 两种布局下路径解析均成功

---

## 2. Tier S1 — 死代码与重复实现清理 (P1)

目标：减少二进制体积、攻击面、维护成本。**所有删除项均经 Grep 验证前端零调用**。

### S1-1 删除死命令（10 个）
| 命令 | 文件:行 | 验证 |
|------|--------|------|
| `transform_variable_name` | tools_cmd.rs:5 | src/ 无 invoke |
| `cycle_variable_name` | tools_cmd.rs:79 | src/ 无 invoke |
| `translate_selection_with_text` | translate.rs:317 | src/ 无 invoke（走 capability） |
| `compare_translate` | translate.rs:772 | src/ 无 invoke（走 ParallelCompare） |
| `ai_context_translate` | ai_cmd.rs:165 | services/ai.ts 无调用 |
| `system_ocr` | capture.rs:1102 | services/ocr.ts 仅用 detailed |
| `capture_full_screen` | capture.rs:426 | 走 snapshot 流程 |
| `detect_text_regions` | capture.rs:1736 | src/ 无 invoke |
| `hook_read_messages` | hook_inject_cmd.rs:319 | 走事件监听 |
| `hook_process_messages` | hook_inject_cmd.rs:335 | 走 host pump 自动 |

**操作**：删除函数 + 删 `generate_handler!` 注册行 + 删配套结构（如 TextRegion）

### S1-2 删除死代码
- `image_translate.rs:581-586`：if/else 两分支返回同值，删条件
- `event_store.rs:231-240`：时间旅行方法（见 S0-7）
- `event_store.rs:97-126`：`init_schema()` 中 card_patches 建表，迁移脚本已有，删运行时 DDL
- `event_store.rs:129-150`：dictionary_history 运行时建表（见 S0-6）
- `dictionary_cmd.rs:741-752`：`ensure_local_dict_tables` 中 oxford_dict/gpt_dict/core_vocabulary 建表，迁移脚本已有，删运行时 DDL

### S1-3 合并 overlay 创建入口
- **文件**：`src-tauri/src/overlay/window_manager.rs:43-151,188-268`、`src-tauri/src/commands/window.rs:348-370,589-628`
- **问题**：4 个 overlay 创建入口（create_overlay_window / _ex / _via_http / update_overlay），逻辑重复且行为略异
- **修复**：收敛为 `create_or_update_overlay(opts)` 单入口，opts 包含 content/level/dismiss/steal_focus/http

### S1-4 合并 WinRT OCR 实现
- **文件**：`src-tauri/src/commands/capture.rs:1102-1179` vs `src-tauri/src/ocr_engine.rs:15-80`
- **问题**：WinRT OCR 流程在两处各自实现
- **修复**：capture 层只做 base64 解码与 detailed 包装，统一委托 `ocr_engine::run_winrt_ocr`

### S1-5 合并 offline_ocr 调用
- **文件**：`src-tauri/src/commands/capture.rs:929-954` vs `src-tauri/src/image_translate.rs:578-604`
- **问题**：两处各自调用 `run_offline_ocr` + `synthetic_ocr_lines_from_text`
- **修复**：抽 `run_offline_ocr_detailed` 公共函数

### S1-6 合并 GetCursorPos 封装
- **文件**：`present.rs:350-361`、`auto_watch.rs:447-464`、`follow_controller.rs:237-259`、`window.rs:317-340`
- **问题**：4 处独立 GetCursorPos 封装，fallback 坐标不同
- **修复**：抽 `crate::win::cursor_pos()` 单一实现

### S1-7 合并 UIA TextPattern 处理
- **文件**：`uiautomation.rs:200-259` vs `hover_pick.rs:629-705`
- **问题**：两套 UIA TextPattern 处理逻辑
- **修复**：抽 `uia_common::read_text_range(range) -> (String, Option<SelectionBounds>)`

---

## 3. Tier S2 — 资源管理与可靠性 (P1)

目标：消除线程泄漏、文件泄漏、内存泄漏。

### S2-1 mouse_hook 线程泄漏修复
- **文件**：`src-tauri/src/selection/mouse_hook.rs:378-446`
- **问题**：install() spawn 的消息循环线程 JoinHandle 被丢弃；uninstall() 只 UnhookWindowsHookEx 不 PostThreadMessage(WM_QUIT)，GetMessageW 永远阻塞
- **修复**：存储 JoinHandle；uninstall 时 `PostThreadMessage(thread_id, WM_QUIT)` 并 join

### S2-2 auto_watch 任务可等待化
- **文件**：`src-tauri/src/selection/auto_watch.rs:50-53,85-94`
- **问题**：run_loop 与 moon-hook-bridge 两个 spawn 的 JoinHandle 立即丢弃，request_stop 无法 await
- **修复**：存储两个 JoinHandle；request_stop 后 await 完成或带 500ms 超时 abort

### S2-3 ocr_offline 临时文件泄漏修复
- **文件**：`src-tauri/src/ocr_offline.rs:27`
- **问题**：`moontranslator_offline_ocr_*.png` 在命令末尾才清理，进程被强杀或 panic 则泄漏
- **修复**：用 `TempFileGuard`（capture.rs 已有该模式）包裹；或改 stdin pipe 传图避免落盘

### S2-4 overlay window_manager document.write 内存泄漏
- **文件**：`src-tauri/src/overlay/window_manager.rs:85-97`
- **问题**：复用 overlay 窗口时用 `document.open(); document.write(html); document.close();`，WebView2 下 document.write after load 泄漏内存且慢
- **修复**：改用 `document.body.innerHTML = ...` 或 `location.replace('data:...')`

### S2-5 overlay 窗口不销毁策略
- **文件**：`src-tauri/src/overlay/window_manager.rs:271-283`
- **问题**：close_overlay_window 仅 hide，从不 destroy；app 长期运行后隐藏 overlay webview 持续占内存
- **修复**：增加配置项 `overlay_destroy_on_close`，或定时（5 分钟未用）destroy 并下次重建

### S2-6 PDF 同步读取修复
- **文件**：`src-tauri/src/pdf.rs:190`
- **问题**：`std::fs::read(file_path)` 将整个 PDF 同步读入内存，几百 MB 扫描版会撑爆
- **修复**：改 `tokio::fs::read` + 大文件分块，或走 `pdfium-render` 流式渲染

### S2-7 dictionary_history 错误吞没修复
- **文件**：`src-tauri/src/commands/dictionary_cmd.rs:257-269`
- **问题**：fire-and-forget spawn，错误 `let _ =` 吞掉，无日志
- **修复**：失败时 `tracing::warn!("dictionary history write failed: {}", e)`

### S2-8 MultiSourceDictionary 重复构造修复
- **文件**：`src-tauri/src/commands/dictionary_cmd.rs:95-106`
- **问题**：`tokio::join!` 两个分支各 `MultiSourceDictionary::new()` 一次，重复构造 HTTP client
- **修复**：提到 join 外共享同一实例

### S2-9 epub_reader 字符串拼接注入修复
- **文件**：`src-tauri/src/epub_reader.rs:261-317`
- **问题**：`replace("{{ORIG}}", ...)` 风格注入，含 `&`/`<` 的文本会破坏 XHTML
- **修复**：至少做 HTML escape；理想用 `scraper` / `html5ever` 做 DOM 级插入

### S2-10 数据库索引补齐
- **文件**：`src-tauri/migrations/003_indexes.sql`（新建）
- **问题**：
  - `card_events` 缺 `(card_id, timestamp)` 复合索引（`load_events` 查询路径）
  - `review_logs` 缺 `session_id` 索引（导出查询路径）
- **修复**：新增迁移脚本补索引

### S2-11 UIA GetText 无界修复
- **文件**：`src-tauri/src/selection/uiautomation.rs:218-219`
- **问题**：`range.GetText(-1)` 请求全部文本，超大文档返回数 MB，阻塞 800ms 超时
- **修复**：限制 maxChars=4096，与 hover_pick.rs:652 的 80/400 上限一致

### S2-12 UIA CoInitializeEx 错误处理
- **文件**：`src-tauri/src/selection/uiautomation.rs:91-96`
- **问题**：RPC_E_CHANGED_MODE 表示 COM 已被其他调用方初始化，不应视为致命
- **修复**：忽略 RPC_E_CHANGED_MODE，仅在 S_FALSE 时继续

---

## 4. Tier S3 — 一致性与 UX (P1/P2)

目标：类型对齐、i18n 覆盖、配置漂移、主题一致。

### S3-1 类型漂移修复（TS ↔ Rust）
| TS 文件:行 | Rust 文件:行 | 问题 | 修复 |
|-----------|-------------|------|------|
| types/index.ts:26-35 | models/config.rs:44 | LlmProviderEntry.apiFormat 缺失 | 补字段（见 S0-5） |
| types/index.ts:49-54 | models/config.rs:213-217 | YoudaoConfig.ocrAppKey/Secret 可选性 | 统一非可选 |
| types/index.ts:57 | models/config.rs:159 | EnginesConfig.caiyun 可选性 | 统一非可选，删 configStore 兜底 |
| types/index.ts:157-237 | models/config.rs:800-938 | AppConfig 字段顺序 | 按相同顺序重排 |
| stores/configStore.ts:142-150 | cache.rs | CacheStats snake_case | 两侧统一 camelCase |

### S3-2 60 处裸 invoke 改 safeInvoke
- **文件**：17 个文件共 60 处裸 `invoke('xxx')`（services/learningMode, statistics, dataIO, dictOptimize, fsrsOptimization, githubExport, notification, modelProvider, vocabulary, wordDetail; pages/VocabularyLearning, VocabularyReview, DictionarySearch; components/OcrRegionFrame, settings/HookProfileSettings, PostProcessSettings, PreProcessSettings）
- **修复**：统一改用 `safeInvoke` / `invokeOrThrow`
- **附加**：从 lib.rs generate_handler! 提取命令名联合类型，让 safeInvoke 的 command 参数类型化

### S3-3 i18n 硬编码中文清理
| 文件:行 | 内容 |
|--------|------|
| pages/settings/engines/enginesMeta.ts:80-195 | nameZh / credentialHint |
| components/AiSettings.tsx:163,169,189,212,224,236,238,247,258,260,285-286,294,307-308,322-323 | 20+ 条 toast/UI 文案 |
| App.tsx:98-99 | "本地词典未加载…" |
| stores/translateStore.ts:171 | "翻译无结果…" |
| stores/configStore.ts:315 | "配置保存失败…" |
| pages/DocumentsViewer.tsx:34,46,57 | "请在桌面应用中打开文件…" |
| pages/MainTranslator.tsx:680 | "收藏到生词本…" |

**修复**：全部走 i18n key；dev 模式 `t()` 找不到 key 时 `console.warn`

### S3-4 死 i18n 键清理
- **文件**：`src/i18n/{zh,en,ja,ko}.json`
- **问题**：`nav.batch`、`nav.plugins`、`nav.marketplace` 及整棵 `plugins`/`marketplace`/`batch` 子树零引用
- **修复**：删除死键缩减 locale 体积

### S3-5 pop_button 主题跟随
- **文件**：`src-tauri/src/selection/pop_button.rs:22-38`
- **问题**：硬编码深色（#12141a / #1a1d27 / #e8eaed），不响应 set_overlay_theme_light
- **修复**：读取 OVERLAY_LIGHT 状态注入 CSS 变量，与 html_builder::theme_css() 统一

### S3-6 epub CSS 主题跟随
- **文件**：`src-tauri/src/epub_reader.rs:172-175`
- **问题**：双语 EPUB CSS 硬编码 `color: #333` / `#1a56db` / `#999`
- **修复**：改用 CSS 变量或 epub-text 语义类

### S3-7 AI 命令 LLM 参数走配置
- **文件**：`src-tauri/src/commands/ai_cmd.rs`（见 S0-3）
- **修复**：用 `config.llm_temperature` / `config.llm_max_tokens` 替换硬编码 0.3/4096

### S3-8 非 OpenAI 流式回退告警
- **文件**：`src-tauri/src/engine/llm.rs:593`
- **问题**：Anthropic/Gemini 静默回退非流式
- **修复**：回退时 emit 警告事件；或实现 Anthropic/Gemini 原生 SSE

### S3-9 polish_translation 与 ai_polish_translation 合并
- **文件**：`translate.rs:690` vs `ai_cmd.rs:53`
- **问题**：功能重复，前者走 façade，后者绕 façade
- **修复**：合并为单一入口，统一走 façade（依赖 S0-3）

### S3-10 query_tm 守卫补齐
- **文件**：`src-tauri/src/commands/translate.rs:756`
- **问题**：仅读 tm_threshold，未检查 tm_enabled
- **修复**：增加 `if !config.tm_enabled { return Ok(None) }`

### S3-11 detect_language 长度校验
- **文件**：`src-tauri/src/commands/translate.rs:490`
- **问题**：未调用 `security::validate_text_length`
- **修复**：增加校验

### S3-12 DocumentsViewer 懒加载
- **文件**：`src/pages/DocumentsViewer.tsx:6-11`
- **问题**：PdfViewer/EpubViewer/SubtitleViewer/OfficeViewer/ImageFileTranslate/Glossary 全 eager
- **修复**：按 DocKind 动态 import

### S3-13 OcrScreenshot 组件懒加载
- **文件**：`src/App.tsx:14-17`
- **问题**：OcrScreenshotSelector/OcrRegionFrame/OcrScreenshotTranslator eager
- **修复**：改 `lazy()`

### S3-14 toast 定时器泄漏
- **文件**：`src/stores/toastStore.ts:31-35`
- **问题**：removeToast/clearAll 不清理 setTimeout，孤儿定时器仍触发
- **修复**：removeToast 中 clearTimeout

### S3-15 unsafe SAFETY 注释批量补充
- **文件**：约 30 处（selection/, security.rs DPAPI, hover_pick.rs, capture.rs, window.rs, process_list.rs）
- **修复**：按 rustc unsafe-code-guidelines 规范补充 `// SAFETY:` 注释

### S3-16 println 替 tracing
- **文件**：`src-tauri/src/infrastructure/event_store.rs:152`
- **问题**：`println!("✅ Event Store schema initialized")` 进 stdout 不进结构化日志
- **修复**：替换 `tracing::info!`

---

## 5. Tier S4 — 工程化与发布 (P1)

目标：CI 可复现、安全基线、版本对齐。

### S4-1 CI runner 标签修复
- **文件**：`.github/workflows/ci.yml:20,86,115`、`release.yml:17,69,110`、`rust-remote.yml`
- **问题**：`windows-2025-vs2026` 非标准 GitHub-hosted 标签，fork/外部贡献者无法复现
- **修复**：改 `windows-latest` 或显式标注 self-hosted

### S4-2 CI actions 版本修复
- **文件**：同上
- **问题**：`actions/checkout@v6`、`setup-node@v6`、`cache@v5`、`upload-artifact@v7`、`download-artifact@v8` 在 Marketplace 不存在（当前最高 v4）
- **修复**：回退到现存稳定主版本

### S4-3 tauri.conf.json CSP 修复
- **文件**：`src-tauri/tauri.conf.json:27`
- **问题**：`"csp": null` webview 无 CSP，可被远程脚本注入
- **修复**：设置 `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'`（unsafe-inline for tauri injected styles）

### S4-4 extension 版本对齐
- **文件**：`extension/manifest.json:5` vs `package.json:4` vs `tauri.conf.json:3`
- **问题**：扩展 1.0.0、桌面 0.1.0 分叉
- **修复**：统一版本源（读 package.json）

### S4-5 extension lint 覆盖
- **文件**：`package.json:13`
- **问题**：lint 只扫 src，extension/ 仅靠 node --check
- **修复**：把 extension 纳入 ESLint

---

## 6. Tier S5 — 功能缺口（P0/P1，较大，可延后到 v2.5）

目标：填补审计发现的未完成功能。**这些项工作量较大，可在 S0-S4 完成后视情况推进**。

### S5-1 PDF 双语导出实现
- **文件**：`src-tauri/src/commands/pdf_cmd.rs:114-119`
- **问题**：`translate_pdf` 仅返回内存结构，无 .pdf 导出（与 EPUB `save_bilingual_epub` 不对称）
- **修复**：依赖 v1 Tier 1.5 的 IL + per-char writeback；或短期用 PDFMathTranslate 子进程桥接
- **关联**：v1 Tier 1.5 P1-P10

### S5-2 word_detail 词源分析实现
- **文件**：`src-tauri/src/commands/word_detail_cmd.rs:310`
- **问题**：`// TODO: 集成词根词缀数据库或 LLM 分析`，前端词根区域长期空
- **修复**：接入已存在 `morphology` 表查询，或调用 GPT4 channel 生成

### S5-3 extension 双份引擎合并
- **文件**：`extension/background/service-worker.js:243-445`
- **问题**：完整 Google/Youdao/Microsoft/DeepL/DeepLX/LLM 实现与 Rust engine/ 双份维护
- **修复**：统一改走桌面桥接（`/browser/translate`），删除扩展内 JS 引擎
- **关联**：依赖桌面 api_server 稳定（S4-3 CSP 修复后）

### S5-4 OCR 通道 TM/cache 评估
- **文件**：`src-tauri/src/services/translation.rs:925,1046,1090,1221`
- **问题**：OCR 通道显式跳过 TM 查询与缓存写入（注释 `keep OCR path lean`），导致重复 OCR 同一文本无法复用
- **修复**：对文档 OCR（非实时取词）启用 TM/cache；实时取词保持 lean
- **需决策**：是否对实时 OCR 也启用（可能引入缓存过期问题）

### S5-5 remote_uninstall RVA 校验
- **文件**：`src-tauri/src/hook_inject.rs:257-308`
- **问题**：本地 LoadLibraryW + GetProcAddress 算 RVA 再加 remote_module 基址，本地与远程 DLL 版本不同则跳转错误地址
- **修复**：校验本地与远程 DLL 同路径同文件大小；或用 CreateRemoteThread + GetProcAddress(kernel32!GetProcAddress) 远程解析

### S5-6 clipboard 重试 UIA fallback
- **文件**：`src-tauri/src/selection/clipboard.rs:260-268`
- **问题**：open_clipboard_retry 10×100ms 失败后无 fallback
- **修复**：失败时回退 UIA selection provider

### S5-7 OCR 几何常量单源
- **文件**：`src-tauri/src/ocr_region_consts.rs` vs `src/components/ocrRegionGeometry.ts`
- **问题**：两侧常量需手工同步，无单测校验
- **修复**：构建期代码生成或 snapshot 测试断言一致

### S5-8 hook_text 单字 CJK 过滤放宽
- **文件**：`src-tauri/hook-dll/src/hook_text.cpp:116-125`
- **问题**：IsPrintableText 要求 char_count >= 2，单字 CJK（如"日"）被错误过滤
- **修复**：CJK 范围放宽到 >=1

### S5-9 OEM 编码正确解码
- **文件**：`src-tauri/src/selection/clipboard.rs:288-297`
- **问题**：read_ansi_or_oem 调用 GetOEMCP() 但结果未用，日文 Shift-JIS OEM 文本乱码
- **修复**：用 encoding crate 按 GetOEMCP() 解码

### S5-10 cache TTL 配置化
- **文件**：`src-tauri/src/cache.rs:70`
- **问题**：`ttl_hours: 72` 硬编码
- **修复**：暴露为 CacheConfig 字段

### S5-11 ASS 字幕双语导出健壮性
- **文件**：`src-tauri/src/subtitle.rs:301-435`
- **问题**：generate_ass_bilingual 字符串拼接替换 ASS Dialogue 行，含逗号/换行易错
- **修复**：用 ASS 解析库或至少做 `\\N` 转义

### S5-12 PDF 引擎探测路径安全
- **文件**：`src-tauri/src/pdf.rs:439-462`
- **问题**：`std::fs::read_to_string` 遍历候选可执行路径，未做 PATH 解析或安全校验
- **修复**：用 `which` crate 解析 + 路径白名单

---

## 7. 实施顺序（本会话内完成 S0 + S1 + S2 + S3 + S4，S5 视情况）

```
Phase 1 (S0 正确性)    │ S0-1 Hook 共享内存 → S0-2 present.rs → S0-3 AI façade →
                       │ S0-4 WinRT spawn_blocking → S0-5 类型漂移 → S0-6 迁移 →
                       │ S0-7 死代码删除 → S0-8 路径收敛
                       │ 验证: cargo check + tsc + cargo test
─────────────────────────────────────────────────────────────────
Phase 2 (S1 死代码)    │ S1-1 删 10 死命令 → S1-2 删死代码 → S1-3 合并 overlay →
                       │ S1-4 合并 WinRT OCR → S1-5 合并 offline_ocr →
                       │ S1-6 合并 GetCursorPos → S1-7 合并 UIA
                       │ 验证: cargo check + tsc + grep 验证零引用
─────────────────────────────────────────────────────────────────
Phase 3 (S2 资源)      │ S2-1 mouse_hook → S2-2 auto_watch → S2-3 ocr_offline →
                       │ S2-4 document.write → S2-5 overlay destroy → S2-6 PDF async →
                       │ S2-7 dict history log → S2-8 MultiSourceDictionary →
                       │ S2-9 epub escape → S2-10 索引 → S2-11 UIA GetText →
                       │ S2-12 CoInitializeEx
                       │ 验证: cargo check + tsc
─────────────────────────────────────────────────────────────────
Phase 4 (S3 一致性)    │ S3-1 类型对齐 → S3-2 safeInvoke 迁移 → S3-3 i18n 清理 →
                       │ S3-4 死 i18n 键 → S3-5 pop_button 主题 → S3-6 epub 主题 →
                       │ S3-7 AI 参数 → S3-8 流式告警 → S3-9 polish 合并 →
                       │ S3-10 query_tm → S3-11 detect_language → S3-12 DocumentsViewer lazy →
                       │ S3-13 OcrScreenshot lazy → S3-14 toast timer → S3-15 SAFETY → S3-16 tracing
                       │ 验证: cargo check + tsc + npm run lint
─────────────────────────────────────────────────────────────────
Phase 5 (S4 工程化)    │ S4-1 CI runner → S4-2 CI actions → S4-3 CSP → S4-4 版本对齐 → S4-5 extension lint
                       │ 验证: YAML 语法 + 版本号一致
─────────────────────────────────────────────────────────────────
Phase 6 (S5 功能缺口)  │ 视剩余工作量推进 S5-2 ~ S5-12（S5-1 PDF 导出延后到 v1 Tier 1.5）
                       │ 验证: 各项功能 smoke
```

**硬依赖**：
- S0-3 (AI façade) 必须在 S3-7 (AI 参数) / S3-9 (polish 合并) 之前
- S0-5 (apiFormat) 必须在 S0-3 之后（AI 命令路由依赖类型）
- S0-6 (迁移) 必须在 S1-2 (删运行时 DDL) 之前
- S1-4 (合并 WinRT OCR) 必须在 S0-4 (spawn_blocking) 之后

---

## 8. 验证门

| Phase | 门 | 标准 |
|-------|----|------|
| S0 | cargo check + tsc + cargo test | 零编译错误，零类型错误，已有测试全过 |
| S1 | grep 验证 | 删除的命令在 src/ 与 src-tauri/ 零引用 |
| S2 | cargo check + tsc | 零编译错误，JoinHandle 全部存储 |
| S3 | cargo check + tsc + npm run lint | 零编译错误，零类型错误，lint 通过 |
| S4 | YAML 语法 + 版本号检查 | CI workflow 可解析，版本号一致 |
| S5 | 各功能 smoke | 视具体项 |

**最终自检**：`cargo check --manifest-path src-tauri/Cargo.toml` + `npx tsc --noEmit` + `npm run lint`（如可用）+ `cargo test`（如有）

---

## 9. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Hook 共享内存契约改动破坏 IAT | 仅调整 sequence 语义，不改内存布局；DLL 与宿主同 PR 更新 |
| AI 命令改走 façade 后行为变化 | 保留旧路径作为 fallback，feature flag 切换 |
| WinRT spawn_blocking 增加 latency | OCR 本就是 IO 密集，阻塞释放 worker 收益 > latency |
| 删死命令误删 | 每条均经 Grep 双重验证 src/ 与 extension/ 零调用 |
| overlay 合并引入回归 | 单入口保留所有旧参数，行为按 opts 矩阵对齐 |
| i18n key 补充工作量 | 仅清理硬编码中文，不新增多语种翻译（en/ja/ko 暂用 zh fallback） |

---

## 10. 不在本计划范围

- **v1 Tier 1 / 1.5 / 2 / 2.5 的功能扩展** — 由 [DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md) 推进
- **AppState 大重构** — 仍冻结，仅做必要 peel
- **Hook DLL IAT 重写** — 仅修共享内存同步，IAT 逻辑不动
- **OCR session 生命周期** — 仍冻结
- **Plugin marketplace** — 永久移除
- **特殊站点适配规则** — v1 Tier 4 P3

---

## 11. 相关文档

- [DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md) — v1 功能扩展路线图
- [HEALTH_AUDIT.md](./HEALTH_AUDIT.md) — 2026-07-25 健康审计
- [MODULE_MAP.md](./MODULE_MAP.md) — 模块地图与重构交接
- [CURRENT_FOCUS.md](./CURRENT_FOCUS.md) — 当前焦点
- [REFERENCE_STUDY.md](./REFERENCE_STUDY.md) — 参考仓库研究 §1-8
- [ENGINE_FACADE_INVENTORY.md](./ENGINE_FACADE_INVENTORY.md) — 引擎 façade 清单

---

**Last updated:** 2026-07-31
**Plan owner:** assistant (待 user 审核)
**Next review:** S0 完成后
