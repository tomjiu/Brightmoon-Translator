# Engine Settings Contract

Authoritative mapping of translation engine ids to enable rules (must match Rust `Router`), credentials, and settings tab ownership.

| id | Enable rule (matches Rust Router) | Credentials | Settings tab |
|----|-----------------------------------|-------------|--------------|
| llm | enabled providers with key (priority failover); else top-level `all_keys()` / FE: apiKey or apiKeys | apiKey, baseUrl, model; optional `providers[]` | AI 增强 (edit) + 翻译引擎 (summary) |
| google | `engines.google.enabled` | none | 翻译引擎 |
| youdao | `engines.youdao.enabled` | optional OCR keys; **useAi unused by router** | 翻译引擎 |
| caiyun | enabled **and** non-empty apiToken | apiToken | 翻译引擎 |
| deepl | enabled **and** non-empty apiKey | apiKey, pro | 翻译引擎 |
| deeplx | `engines.deeplx.enabled` | optional apiKey/pro (show fields) | 翻译引擎 |
| baidu | enabled **and** non-empty appId | appId, secret | 翻译引擎 |
| microsoft | `engines.microsoft.enabled` | none | 翻译引擎 |
| yandex | `engines.yandex.enabled` | none | 翻译引擎 |
| offline | `engines.offline.enabled` | models/modelDir (show autoSwitch) | 翻译引擎 |

**Product rule:** first-party engines only — no external plugin marketplace / scan.

**Notes:**

- `useAi` is **UI: hide or label “未接入路由”** — do not implement Youdao AI routing in the engine-settings cleanup plan.
- Multi-provider `llm.providers`: enabled entries with non-empty key are used by Router (sorted by priority, failover). Top-level keys remain fallback when providers is empty.
