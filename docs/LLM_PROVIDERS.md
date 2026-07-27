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

## 代码

- `LlmProviderEntry.apiFormat`：`src-tauri/src/models/config.rs`
- 请求分发：`src-tauri/src/engine/llm.rs`
- 拉模型 / 测连：`src-tauri/src/commands/model_provider_cmd.rs`
- FE：`src/services/modelProvider.ts`、`src/components/AiSettings.tsx`
