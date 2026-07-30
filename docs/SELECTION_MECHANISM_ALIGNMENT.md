# 取词机制对齐 Easydict — 现状分析与落地方案

> 日期：2026-07-29
> 依据：`tmp/reference/oss/easydict_win32/`（dotnet WinUI3 版）源码逐行对照 + Moon 源码核实
> 结论先行：**核心链路已扎实对齐 Easydict**，真正要补的是几个具体机制节点 + 清理死壳。

---

## 0. 一句话定位

Moon 的 `SelectionProviderManager::get_selection_routed` 就是 Easydict `GetSelectedTextAsync` 的对应物。链路 `foreground_process → strategy → is_process_clipboard_suppressed → try_providers_in_order` 已成型。

---

## 1. 机制逐项对照（参考有 → Moon 实际）

| # | 机制 | Easydict | Moon 现状 | 状态 |
|---|---|---|---|---|
| 1 | 进程分流 | `GetSelectedTextAsync` 内分类 | `process_class.rs` 抽到独立模块 | ✅ |
| 2 | ClipWait + 非文本抑制 | 30ms 轮询 / 5min 抑制 | `clipboard.rs:392-426` + `record_outcome:62` | ✅ |
| 3 | 合成 Ctrl+C 标记 | `EASYDICT_SYNTHETIC_KEY` | `MOON_SYNTHETIC_KEY` `mouse_hook.rs:22` | ✅ |
| 4 | UIA 800ms 超时 | `UiaExecutionTimeoutMs=800` | `uiautomation.rs:56` | ✅ |
| 5 | Pop 只信 pending | `_pendingText` | `pop_button::take_pending` + `auto_watch.rs:297` | ✅ |
| 6 | 词/译分展示 | Mini 按内容 | `present::present_selection` 路由 | ✅ |
| 7 | 选区=真 GetSelection | 只 selection range | `uiautomation.rs:105` 永不返回全文 | ✅ |
| 8 | 剪贴板恢复矩阵 | `ResolveClipboardRestore` | `clipboard.rs:93` + 测试 `:537` | ✅ |
| 9 | **UIA 忙时跳过** | `SemaphoreSlim(1,1)` + 200ms | `uiautomation.rs:33` `UIA_SEMAPHORE` | ✅ 已补 |
| 10 | **UIA 成功平反 suppress** | `RecordOutcome(Success)` | `manager.rs:106` → `clipboard::record_selection_success` | ✅ 已补 |
| 11 | Electron 同次不二次 C | 显式 flag | 路由顺序 + 早返回 | ✅ 等价 |
| 12 | 取词失败=静默 | 空则不弹 | `classify_text` Junk 拒 + `accept_for_pop` | ✅ |
| 13 | 悬停分词 | MTT 词边界 | `hover_pick.rs:667` TextPattern 套 `extract_word_candidate_with_hint` | ✅ 已补 |

---

## 2. 四条对齐任务

### 任务 1 — 统一取词入口（对齐 GetSelectedTextAsync）

- **1a. UIA 信号量**：`uiautomation.rs` 加 `Semaphore(1,1)` + 200ms 等待
- **1b. UIA 成功平反 suppress**：`record_selection_success` 在 UIA 成功时调用
- **1c. 同次不二次 Ctrl+C**：已由 `try_providers_in_order` 早返回满足
- **1d. replace 路径用 routed**：`input_replacement_impl.rs` 改用 `get_selection_routed`，动态读 exclude

### 任务 2 — 取词失败契约

核实结论：**已满足**。四条入口（hotkey/pop/auto/hover/replace）对 None 都 Err/不弹。

### 任务 3 — Pop pending 同串契约 + 日志

`take_pending` 加消费日志（len+预览+age_ms），与 `show` 的 armed 日志对齐。

### 任务 4 — 悬停取词套 MTT 词边界

TextPattern 词路径改用 `extract_word_candidate_with_hint`，与 ValuePattern 路径统一。

---

## 3. 死壳清理

- 删除 4 个死命令 + `lib.rs` 注册
- 删除 3 个零实现 trait + `EmbeddedAppType` + `classify_embedded_app` + `AppContext` 死字段
- 删除 4 个死函数：`get_selection_excluding` / `show_selection_translate_text_public` / `format_dict_body` / `get_selection()`
- `estimate_mt_card_size` 统一双 overlay 管线尺寸（CJK-aware）

---

## 4. 验收口径

- **错字率**：浏览器/编辑器/终端三类应用划词，取到文本与实际选区一致
- **空弹率**：空选区/无选区触发不产生弹窗或翻译
- **乱译率**：取词正确→译文正确；取词失败→静默
- **并发**：hotkey + auto_select 同时触发，UIA 不争抢（日志 `semaphore busy`）
- **Pop 一致性**：show 与 take 日志预览逐次一致
