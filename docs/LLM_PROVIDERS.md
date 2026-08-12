# LLM 提供商配置

降低配置成本：内置常见云厂商模板 + **三种请求格式** + 模型列表拉取。

## 请求格式（apiFormat）

| 格式 | 鉴权 | 对话 | 拉模型 |
|------|------|------|--------|
| `openai` | `Authorization: Bearer` | `{base}/chat/completions` | `GET {base}/models` |
| `anthropic` | `x-api-key` + `anthropic-version` | `{base}/messages` | `GET {base}/models` 或静态列表 |
| `gemini` | `?key=` | `{base}/models/{id}:generateContent` | `GET {base}/models` 或静态列表 |

自定义服务商（OneAPI / NewAPI / 自建网关）多数选 **openai**。

流式翻译目前仅 **openai** 路径完整支持；anthropic / gemini 走非流式整段返回。

## 设置路径

AI 设置 / LLM 模型配置 → 点预设（DeepSeek、Claude、Gemini…）→ 填 API Key → **拉取模型** → 选模型 → 保存。

## AI 学习专用模型（与翻译隔离）

翻译 / 润色 / 词典走 `llmProviderId` 对应 provider；**AI 学习系统**（学习模式词表、词汇卡片词源/助记/例句、复习计划）可另选 `learn_llm_provider_id`，通过 `provider_from_config_for_learning` 独立渲染。

- 学习专用模型**不会**命中 `AiTranslateTools` 的提示词（润色 / 术语 / 风格仅作用于翻译引擎）。
- 未选择学习专用模型时回退到普通 provider。
- 配置项：`AppConfig.learn_llm_provider_id`（`config.rs`）；选择器在 AI 设置页 `AiSettings.tsx`。

## 代码

- `LlmProviderEntry.apiFormat`：`src-tauri/src/models/config.rs`
- 请求分发：`src-tauri/src/engine/llm.rs`
- 拉模型 / 测连：`src-tauri/src/commands/model_provider_cmd.rs`
- FE：`src/services/modelProvider.ts`、`src/components/AiSettings.tsx`
