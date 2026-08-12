# Moon Translator — feature checklist

References: immersive-translate, LunaTranslator, read-frog, pot-desktop, STranslate  
Integration matrix: `docs/integration-plan.md`. Live compass: `docs/CURRENT_FOCUS.md`, `docs/MODULE_MAP.md`.

Status is **code-truth as of 2026-08-12** (not marketing). OCR GUI smoke still manual.

**Product:** first-party only — no plugin marketplace / external plugin host.

---

## 1. Desktop

### 1.1 Main translator
| Feature | Status | Notes |
|---------|--------|-------|
| Multi-line input + debounce | ✅ | ~500ms |
| Auto detect / lang swap / copy | ✅ | |
| Multi-engine results | ✅ | Via routing strategy |
| Stream (LLM token stream) | ✅ | `translate_stream` + MainTranslator |
| History | ✅ | |
| Global hotkeys | ✅ | Config-driven; needs `loadConfig` (fixed) |
| System tray | ✅ | Tray menu in `lib.rs` |

### 1.2 OCR screenshot
| Feature | Status | Notes |
|---------|--------|-------|
| Fullscreen snapshot + region | ✅ | Single path: selector → region frame |
| WinRT / Youdao / tesseract.js | ✅ | |
| Continuous refresh | ✅ | **Default OFF** (I1) |
| Follow window | ✅ | I6 |
| Box/flicker residuals | ⚠️ | See `OCR_INVARIANTS.md`; smoke later |
| Layout analysis (merge/split) | ⏳ medium | Needed; not urgent |
| Overlay visual polish | ⏳ low | Font/theme/paint order |
| Recognize workspace page | ⏳ maybe | Prefer mature OCR backends over pot-style IDE |

### 1.3 Engines
| Feature | Status | Notes |
|---------|--------|-------|
| LLM / Google / Baidu / Youdao / DeepL / … | ✅ | Multiple providers |
| Parallel / fallback strategies | ✅ | Config `routingStrategy` |
| Translation cache / TM | ✅ | Cache + TM commands/UI |
| Unified façade | ⚠️ | Multiple entrypoints — `MODULE_MAP` B |
| External plugins / marketplace | ❌ removed | First-party engines only |
| Offline auto_switch fallback | ✅ | All strategies fail → offline (chain complete) |
| Offline chain query + modelDir | ✅ | Settings tab: pivot chain + gap + dir input |

### 1.4 Hook
| Feature | Status | Notes |
|---------|--------|-------|
| UIA + clipboard monitor | ✅ | Default sources |
| DLL inject (H-Code) | ⚠️ experimental | **Next major** after OCR+划词 stable (shell apps) |
| Profiles UI | ✅ | Not fully applied at monitor start |

### 1.5 词典 / AI 学习 / 历史（顶级导航）
| Feature | Status | Notes |
|---------|--------|-------|
| 顶级导航拆分 | ✅ | `App.tsx` navItems：翻译 / Hook / 文档 / 词典 / AI 学习 / 历史 / 设置 |
| 词典页：查询 / 生词本 / 词典优化 | ✅ | `pages/Vocabulary.tsx` |
| AI 学习：学习 / 复习 / 练习 / 统计 / FSRS / 数据 | ✅ | `pages/Study.tsx` |
| 历史页：翻译记忆 / 翻译指标 | ✅ | `pages/History.tsx`（原 tm + metrics 合并） |
| 词典查询历史隔离 | ✅ | `lookup_word_multi_source` 加 `record_history`；悬浮词典传 `false` 不记录 |
| AI 学习专用 LLM | ✅ | 大模型页选择器 `learn_llm_provider_id`，与翻译配置隔离 |
| 翻译提示词 / 润色 / 术语 / 风格 | ✅ | `AiTranslateTools` 位于大模型页，不作用于学习专用模型 |
| 词典优化加载性能 | ✅ | 启动补建 `idx_stardict_frq`（ECDICT stardict），词频统计毫秒级 |

---

## 2. Browser extension

| Feature | Status | Notes |
|---------|--------|-------|
| Selection popup | ✅ | |
| Page translate | ✅ | |
| Hover translate | ✅ | `hover-translator.js` |
| Desktop bridge `:60828` | ⚠️ | Server **default off** — enable in Advanced |
| Batch / context / queues (depth) | ⏳ need | When fine-tuning browser side |
| Bilingual / PDF / YT | ❌ | vs immersive-translate |

---

## 3. Explicitly frozen / secondary / skip

| Item | Status |
|------|--------|
| Plugin marketplace | ❌ removed (first-party only) |
| Floating multi-engine popup | Skip (main window multi-engine enough) |
| Vocabulary boom / Cloudflare multi-end / AppState rewrite | Frozen non-goals |
| Extra local HTTP ExternalCall polish | Low — last, no arch churn |
| Luna-class inject deep product | After OCR + 划词 |

See [CURRENT_FOCUS.md](./docs/CURRENT_FOCUS.md) tiers.

---

## 4. Priority (now)

1. Keep desktop OCR + 划词 stable (smoke when free)  
2. Extension **depth** when browser phase starts  
3. OCR layout analysis (medium) → overlay polish (low)  
4. Inject/Hook major vertical after 1  
5. HTTP control extras last
