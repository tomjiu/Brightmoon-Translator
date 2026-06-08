# Moon Translator 插件开发指南

本文档介绍如何为 Moon Translator 开发插件。

---

## 插件架构

Moon Translator 使用基于 HTTP 端点的插件架构。插件是一个独立的服务，通过 HTTP API 与主应用通信。

```
┌─────────────────────────────────────────────────────────┐
│                    Moon Translator                       │
│  ┌─────────────────────────────────────────────────┐    │
│  │              插件管理器                           │    │
│  │  - 扫描 plugins 目录                            │    │
│  │  - 读取 manifest.json                           │    │
│  │  - 注册到翻译引擎                               │    │
│  └─────────────────────────────────────────────────┘    │
│                         │                                │
│                         │ HTTP                           │
│                         ▼                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  插件 A     │  │  插件 B     │  │  插件 C     │     │
│  │ (翻译)      │  │ (OCR)       │  │ (TTS)       │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
```

### 插件类型

| 类型 | 说明 | 状态 |
|------|------|------|
| `translation` | 翻译插件 | 已实现 |
| `ocr` | OCR 插件 | 计划中 |
| `tts` | TTS 插件 | 计划中 |

---

## 插件目录结构

插件存放在 `%APPDATA%/moontranslator/plugins/` 目录下:

```
plugins/
├── my-translator/
│   ├── manifest.json        # 插件清单
│   └── server.js            # 插件服务 (可选)
├── another-translator/
│   ├── manifest.json
│   └── server.py
└── ...
```

---

## manifest.json 格式

每个插件必须包含一个 `manifest.json` 文件:

### 基本结构

```json
{
  "name": "My Translator",
  "version": "1.0.0",
  "description": "自定义翻译插件",
  "author": "Your Name",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:8080/translate",
    "supportedLanguages": [
      ["en", "zh"],
      ["zh", "en"],
      ["en", "ja"]
    ],
    "headers": {
      "Authorization": "Bearer xxx"
    }
  }
}
```

### 字段说明

#### 基本字段

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | 是 | 插件名称 |
| `version` | string | 是 | 版本号 (语义化版本) |
| `description` | string | 否 | 插件描述 |
| `author` | string | 否 | 作者 |
| `type` | string | 是 | 插件类型: `translation`, `ocr`, `tts` |
| `enabled` | boolean | 否 | 是否启用 (默认: true) |

#### 翻译插件字段 (`translation`)

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `endpoint` | string | 是 | 翻译 API 端点 URL |
| `supportedLanguages` | string[][] | 否 | 支持的语言对 (空数组表示支持所有) |
| `headers` | object | 否 | 自定义 HTTP 请求头 |

---

## 插件 API

### 翻译插件 API

翻译插件需要实现一个 HTTP POST 端点。

#### 请求格式

```http
POST /translate
Content-Type: application/json

{
  "text": "Hello World",
  "from": "en",
  "to": "zh"
}
```

**字段说明**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `text` | string | 待翻译文本 |
| `from` | string | 源语言代码 ("auto" 表示自动检测) |
| `to` | string | 目标语言代码 |

#### 响应格式

```json
{
  "translated": "你好世界"
}
```

**字段说明**:

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `translated` | string | 是 | 翻译结果 |

#### 错误响应

HTTP 状态码:
- `200`: 成功
- `400`: 请求参数错误
- `500`: 内部错误

错误响应体:
```json
{
  "error": "Translation failed"
}
```

---

## 示例插件

### 示例 1: Node.js 翻译插件

#### 目录结构

```
my-node-translator/
├── manifest.json
├── server.js
└── package.json
```

#### manifest.json

```json
{
  "name": "My Node Translator",
  "version": "1.0.0",
  "description": "使用 Node.js 实现的翻译插件",
  "author": "Your Name",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:3001/translate",
    "supportedLanguages": [],
    "headers": {}
  }
}
```

#### package.json

```json
{
  "name": "my-node-translator",
  "version": "1.0.0",
  "dependencies": {
    "express": "^4.18.0"
  }
}
```

#### server.js

```javascript
const express = require("express");
const app = express();

app.use(express.json());

app.post("/translate", async (req, res) => {
  const { text, from, to } = req.body;

  // 验证请求
  if (!text || !to) {
    return res.status(400).json({ error: "Missing required fields" });
  }

  try {
    // 在这里实现你的翻译逻辑
    const translated = await yourTranslateFunction(text, from, to);

    res.json({ translated });
  } catch (error) {
    console.error("Translation error:", error);
    res.status(500).json({ error: error.message });
  }
});

async function yourTranslateFunction(text, from, to) {
  // 示例: 调用外部翻译 API
  const response = await fetch("https://api.example.com/translate", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ text, source: from, target: to }),
  });

  const data = await response.json();
  return data.translatedText;
}

const PORT = 3001;
app.listen(PORT, () => {
  console.log(`Translation plugin running on port ${PORT}`);
});
```

---

### 示例 2: Python 翻译插件

#### 目录结构

```
my-python-translator/
├── manifest.json
├── server.py
└── requirements.txt
```

#### manifest.json

```json
{
  "name": "My Python Translator",
  "version": "1.0.0",
  "description": "使用 Python 实现的翻译插件",
  "author": "Your Name",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:8000/translate",
    "supportedLanguages": [
      ["en", "zh"],
      ["zh", "en"]
    ],
    "headers": {}
  }
}
```

#### requirements.txt

```
fastapi>=0.100.0
uvicorn>=0.23.0
httpx>=0.24.0
```

#### server.py

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
import httpx

app = FastAPI()


class TranslateRequest(BaseModel):
    text: str
    from_lang: str = "auto"
    to: str


class TranslateResponse(BaseModel):
    translated: str


@app.post("/translate", response_model=TranslateResponse)
async def translate(request: TranslateRequest):
    if not request.text or not request.to:
        raise HTTPException(status_code=400, detail="Missing required fields")

    try:
        translated = await your_translate_function(
            request.text, request.from_lang, request.to
        )
        return TranslateResponse(translated=translated)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


async def your_translate_function(text: str, from_lang: str, to: str) -> str:
    # 示例: 调用外部翻译 API
    async with httpx.AsyncClient() as client:
        response = await client.post(
            "https://api.example.com/translate",
            json={"text": text, "source": from_lang, "target": to},
        )
        data = response.json()
        return data["translatedText"]


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
```

---

### 示例 3: Go 翻译插件

#### 目录结构

```
my-go-translator/
├── manifest.json
├── main.go
└── go.mod
```

#### manifest.json

```json
{
  "name": "My Go Translator",
  "version": "1.0.0",
  "description": "使用 Go 实现的翻译插件",
  "author": "Your Name",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:8080/translate",
    "supportedLanguages": [],
    "headers": {}
  }
}
```

#### main.go

```go
package main

import (
    "encoding/json"
    "fmt"
    "io"
    "net/http"
)

type TranslateRequest struct {
    Text string `json:"text"`
    From string `json:"from"`
    To   string `json:"to"`
}

type TranslateResponse struct {
    Translated string `json:"translated"`
}

type ErrorResponse struct {
    Error string `json:"error"`
}

func translateHandler(w http.ResponseWriter, r *http.Request) {
    if r.Method != http.MethodPost {
        w.WriteHeader(http.StatusMethodNotAllowed)
        json.NewEncoder(w).Encode(ErrorResponse{Error: "Method not allowed"})
        return
    }

    var req TranslateRequest
    if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
        w.WriteHeader(http.StatusBadRequest)
        json.NewEncoder(w).Encode(ErrorResponse{Error: "Invalid JSON"})
        return
    }

    if req.Text == "" || req.To == "" {
        w.WriteHeader(http.StatusBadRequest)
        json.NewEncoder(w).Encode(ErrorResponse{Error: "Missing required fields"})
        return
    }

    translated, err := yourTranslateFunction(req.Text, req.From, req.To)
    if err != nil {
        w.WriteHeader(http.StatusInternalServerError)
        json.NewEncoder(w).Encode(ErrorResponse{Error: err.Error()})
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(TranslateResponse{Translated: translated})
}

func yourTranslateFunction(text, from, to string) (string, error) {
    // 示例: 调用外部翻译 API
    // 在这里实现你的翻译逻辑
    return "translated text", nil
}

func main() {
    http.HandleFunc("/translate", translateHandler)

    port := 8080
    fmt.Printf("Translation plugin running on port %d\n", port)
    if err := http.ListenAndServe(fmt.Sprintf(":%d", port), nil); err != nil {
        fmt.Printf("Server error: %v\n", err)
    }
}
```

---

### 示例 4: 使用 OpenAI API 的翻译插件

这是一个更实际的例子，使用 OpenAI 兼容的 API 进行翻译。

#### manifest.json

```json
{
  "name": "OpenAI Translator",
  "version": "1.0.0",
  "description": "使用 OpenAI API 的翻译插件",
  "type": "translation",
  "enabled": true,
  "translation": {
    "endpoint": "http://localhost:3000/translate",
    "supportedLanguages": [],
    "headers": {}
  }
}
```

#### server.js (Node.js)

```javascript
const express = require("express");
const app = express();

app.use(express.json());

const OPENAI_API_KEY = process.env.OPENAI_API_KEY || "sk-xxx";
const OPENAI_BASE_URL =
  process.env.OPENAI_BASE_URL || "https://api.openai.com/v1";
const MODEL = process.env.MODEL || "gpt-3.5-turbo";

const LANGUAGE_NAMES = {
  zh: "中文",
  en: "English",
  ja: "日本語",
  ko: "한국어",
  fr: "Français",
  de: "Deutsch",
  es: "Español",
  ru: "Русский",
};

app.post("/translate", async (req, res) => {
  const { text, from, to } = req.body;

  if (!text || !to) {
    return res.status(400).json({ error: "Missing required fields" });
  }

  try {
    const translated = await translateWithOpenAI(text, from, to);
    res.json({ translated });
  } catch (error) {
    console.error("Translation error:", error);
    res.status(500).json({ error: error.message });
  }
});

async function translateWithOpenAI(text, from, to) {
  const fromName = LANGUAGE_NAMES[from] || from;
  const toName = LANGUAGE_NAMES[to] || to;

  const systemPrompt = `You are a professional translator. Translate the following text from ${fromName} to ${toName}. Only return the translated text, nothing else.`;

  const response = await fetch(`${OPENAI_BASE_URL}/chat/completions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${OPENAI_API_KEY}`,
    },
    body: JSON.stringify({
      model: MODEL,
      messages: [
        { role: "system", content: systemPrompt },
        { role: "user", content: text },
      ],
      temperature: 0.3,
    }),
  });

  if (!response.ok) {
    const error = await response.text();
    throw new Error(`OpenAI API error: ${error}`);
  }

  const data = await response.json();
  return data.choices[0].message.content.trim();
}

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
  console.log(`OpenAI Translator plugin running on port ${PORT}`);
});
```

**使用方法**:

```bash
# 设置环境变量
export OPENAI_API_KEY="sk-xxx"
export MODEL="gpt-4"

# 安装依赖
npm install express

# 启动插件
node server.js
```

---

## 高级功能

### 1. 流式翻译

对于 LLM 类型的翻译插件，可以支持流式输出:

#### 请求格式

```json
{
  "text": "Hello World",
  "from": "en",
  "to": "zh",
  "stream": true
}
```

#### 响应格式 (SSE)

```
data: {"chunk": "你"}
data: {"chunk": "好"}
data: {"chunk": "世界"}
data: [DONE]
```

### 2. 批量翻译

支持一次翻译多段文本:

#### 请求格式

```json
{
  "texts": ["Hello", "World"],
  "from": "en",
  "to": "zh"
}
```

#### 响应格式

```json
{
  "translations": ["你好", "世界"]
}
```

### 3. 上下文感知

支持传入上下文以提高翻译质量:

#### 请求格式

```json
{
  "text": "Hello World",
  "from": "en",
  "to": "zh",
  "context": [
    {
      "source": "Previous sentence",
      "translation": "前一句的翻译"
    }
  ]
}
```

---

## 最佳实践

### 1. 错误处理

- 始终返回有意义的错误信息
- 使用适当的 HTTP 状态码
- 记录错误日志便于调试

### 2. 性能优化

- 实现请求超时
- 使用连接池
- 缓存常用翻译结果
- 支持并发请求

### 3. 安全性

- 验证输入数据
- 保护 API 密钥
- 限制请求频率
- 使用 HTTPS (生产环境)

### 4. 可维护性

- 编写清晰的文档
- 使用语义化版本号
- 提供配置选项
- 支持日志级别调整

---

## 调试技巧

### 1. 手动测试插件

使用 curl 测试插件:

```bash
curl -X POST http://localhost:3001/translate \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello", "from": "en", "to": "zh"}'
```

### 2. 查看插件日志

插件的标准输出会显示在插件的控制台中。

### 3. 检查插件状态

在 Moon Translator 的插件页面可以查看:
- 已发现的插件列表
- 插件启用/禁用状态
- 插件配置信息

### 4. 常见问题

**插件未显示**:
- 检查 manifest.json 格式是否正确
- 确认插件目录在正确的位置
- 重启应用

**翻译失败**:
- 检查插件服务是否运行
- 确认端口是否正确
- 查看插件日志

**语言对不支持**:
- 检查 supportedLanguages 配置
- 空数组表示支持所有语言

---

## 发布插件

目前插件通过本地目录加载，未来可能支持:
- 插件市场
- 在线安装
- 自动更新

---

## 参考资源

- [Moon Translator 源代码](https://github.com/your-username/moontranslator)
- [manifest.json 完整示例](#manifestjson-格式)
- [翻译插件 API 规范](#翻译插件-api)
