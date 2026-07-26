// AI 代理服务 - 代理 LLM 请求

export interface AiRequest {
  word: string;
  context?: {
    translation?: string;
    definitions?: string[];
  };
}

export interface AiResponse {
  mnemonics?: Array<{
    mnemonic_type: string;
    content: string;
    score?: number;
  }>;
  etymology?: {
    origin: string;
    root_breakdown: Array<{
      part: string;
      meaning: string;
      examples: string[];
    }>;
  };
  examples?: Array<{
    text: string;
    context: string;
  }>;
}

export class AiProxy {
  private apiKey: string;
  private baseUrl: string;
  private model: string;
  private cache: KVNamespace;

  constructor(apiKey: string, baseUrl: string, model: string, cache: KVNamespace) {
    this.apiKey = apiKey;
    this.baseUrl = baseUrl;
    this.model = model;
    this.cache = cache;
  }

  // 生成单词助记内容
  async generateContent(word: string, context?: any): Promise<AiResponse | null> {
    // 先查缓存
    const cacheKey = `ai:gen:${word.toLowerCase()}`;
    const cached = await this.cache.get(cacheKey, 'json');
    if (cached) {
      return cached as AiResponse;
    }

    // 构造 prompt
    const prompt = this.buildPrompt(word, context);

    try {
      const response = await fetch(`${this.baseUrl}/v1/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${this.apiKey}`,
        },
        body: JSON.stringify({
          model: this.model,
          messages: [
            {
              role: 'system',
              content: '你是一个英语词汇学习助手，擅长创造记忆方法、分析词源、提供例句。请用 JSON 格式返回结果。',
            },
            {
              role: 'user',
              content: prompt,
            },
          ],
          temperature: 0.7,
          max_tokens: 500,
        }),
      });

      if (!response.ok) {
        console.error('AI API error:', response.status);
        return null;
      }

      const data = await response.json() as any;
      const content = data.choices?.[0]?.message?.content;

      if (!content) {
        return null;
      }

      // 解析 JSON
      const result = this.parseAiResponse(content);

      // 缓存 30 天
      if (result) {
        await this.cache.put(cacheKey, JSON.stringify(result), {
          expirationTtl: 30 * 24 * 3600,
        });
      }

      return result;
    } catch (error) {
      console.error('AI generation error:', error);
      return null;
    }
  }

  // 构造 prompt
  private buildPrompt(word: string, context?: any): string {
    let prompt = `请为英语单词 "${word}" 生成记忆辅助内容，返回 JSON 格式：`;

    if (context?.translation) {
      prompt += `\n中文释义：${context.translation}`;
    }

    if (context?.definitions?.length) {
      prompt += `\n英文释义：${context.definitions.join('; ')}`;
    }

    prompt += `\n\n请返回以下 JSON 结构：
{
  "mnemonics": [
    {
      "mnemonic_type": "etymology|scene|谐音|联想",
      "content": "记忆方法描述",
      "score": 0.8
    }
  ],
  "etymology": {
    "origin": "词源说明",
    "root_breakdown": [
      {
        "part": "词根/前缀/后缀",
        "meaning": "含义",
        "examples": ["同根词示例"]
      }
    ]
  },
  "examples": [
    {
      "text": "英文例句",
      "context": "使用场景说明"
    }
  ]
}`;

    return prompt;
  }

  // 解析 AI 响应
  private parseAiResponse(content: string): AiResponse | null {
    try {
      // 尝试提取 JSON
      const jsonMatch = content.match(/\{[\s\S]*\}/);
      if (!jsonMatch) {
        return null;
      }

      const result = JSON.parse(jsonMatch[0]);

      // 验证结构
      return {
        mnemonics: Array.isArray(result.mnemonics) ? result.mnemonics : undefined,
        etymology: result.etymology || undefined,
        examples: Array.isArray(result.examples) ? result.examples : undefined,
      };
    } catch (error) {
      console.error('Failed to parse AI response:', error);
      return null;
    }
  }
}
