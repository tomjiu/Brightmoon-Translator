# Moon Translator - API接口规范

**版本**：v1  
**Base URL**：`https://api.moontranslator.app/v1` 或 `https://your-workers.your-domain.workers.dev/v1`  
**更新时间**：2026-06-17

---

## 🔑 认证方式

### 用户认证（可选）
部分端点需要用户认证，支持两种方式：

1. **Bearer Token（推荐）**
   ```http
   Authorization: Bearer <jwt_token>
   ```

2. **设备ID（匿名模式）**
   ```http
   X-Device-ID: <uuid>
   ```

---

## 📖 词典查询

### 1. 查询单词（多源聚合）

**端点**：`GET /word/:word`

**描述**：聚合ECDICT、有道、DictionaryAPI.dev等多源词典数据

**参数**：
- `word` (path, required)：要查询的单词
- `sources` (query, optional)：指定数据源，逗号分隔。可选值：`ecdict,youdao,online,oxford,gpt`。默认：全部

**示例请求**：
```http
GET /v1/word/abandon?sources=ecdict,youdao
```

**响应示例**：
```json
{
  "word": "abandon",
  "phonetics": [
    {
      "text": "əˈbændən",
      "audio": "https://dict.youdao.com/dictvoice?audio=abandon&type=2",
      "source": "ECDICT"
    }
  ],
  "chineseTranslation": "v. 放弃；抛弃；n. 放纵",
  "englishDefinitions": [
    "to leave somebody, especially somebody you are responsible for, with no intention of returning",
    "to leave a thing or place, especially because it is impossible or dangerous to stay"
  ],
  "collinsEntries": [
    {
      "pos": "VERB",
      "posCn": "动词",
      "englishDef": "If you abandon a place, thing, or person, you leave the place, thing, or person permanently or for a long time.",
      "examples": [
        {
          "en": "He claimed that his parents had abandoned him.",
          "zh": "他声称他的父母抛弃了他。"
        }
      ]
    }
  ],
  "examples": [
    {
      "en": "The crew abandoned the sinking ship.",
      "zh": "船员们弃船逃生。"
    }
  ],
  "usAudioUrl": "https://dict.youdao.com/dictvoice?audio=abandon&type=2",
  "ukAudioUrl": "https://dict.youdao.com/dictvoice?audio=abandon&type=1",
  "imageUrl": "https://images.unsplash.com/photo-1504280392369-61ec28e4e076",
  "sources": ["ECDICT", "有道", "柯林斯"]
}
```

**错误响应**：
```json
{
  "error": "Word not found",
  "code": "WORD_NOT_FOUND"
}
```

---

### 2. 搜索建议（自动完成）

**端点**：`GET /word/suggest`

**描述**：前缀匹配搜索，返回单词列表 + 中文释义预览

**参数**：
- `q` (query, required)：搜索关键词（至少2个字符）
- `limit` (query, optional)：返回数量，默认10，最大50

**示例请求**：
```http
GET /v1/word/suggest?q=aban&limit=5
```

**响应示例**：
```json
{
  "suggestions": [
    {
      "word": "abandon",
      "preview": "v. 放弃；抛弃"
    },
    {
      "word": "abandoned",
      "preview": "adj. 被抛弃的；无约束的"
    },
    {
      "word": "abandonment",
      "preview": "n. 放弃；抛弃"
    }
  ],
  "total": 3
}
```

---

### 3. 批量查询（移动端优化）

**端点**：`POST /word/batch`

**描述**：一次查询多个单词（减少网络请求）

**请求体**：
```json
{
  "words": ["abandon", "ability", "about"],
  "fields": ["word", "chineseTranslation", "phonetics"]  // 可选，减少响应体积
}
```

**响应示例**：
```json
{
  "results": [
    {
      "word": "abandon",
      "chineseTranslation": "v. 放弃；抛弃",
      "phonetics": [{"text": "əˈbændən", "source": "ECDICT"}]
    },
    // ...
  ]
}
```

---

## 📚 学习系统

### 4. 获取学习计划列表

**端点**：`GET /user/:userId/plans`

**认证**：必需

**响应示例**：
```json
{
  "plans": [
    {
      "id": "plan-uuid-1",
      "name": "四级词汇",
      "description": "4000词 · 每日30词",
      "totalWords": 4000,
      "dailyTarget": 30,
      "progress": {
        "learnedWords": 250,
        "remainingWords": 3750,
        "completionRate": 6.25
      },
      "createdAt": 1718611200,
      "updatedAt": 1718697600
    }
  ]
}
```

---

### 5. 创建学习计划

**端点**：`POST /user/:userId/plans`

**认证**：必需

**请求体**：
```json
{
  "type": "preset",  // preset | imported | custom
  "targetExam": "cet4",  // cet4 | cet6 | ky | ielts | toefl | gre
  "dailyTarget": 30,
  "words": []  // type=imported时必填
}
```

**响应**：
```json
{
  "planId": "plan-uuid-1",
  "wordsCount": 4000,
  "message": "计划创建成功"
}
```

---

### 6. 获取今日学习单词

**端点**：`GET /user/:userId/plans/:planId/today`

**认证**：必需

**响应示例**：
```json
{
  "words": [
    {
      "word": "abandon",
      "wordOrder": 1,
      "learned": false
    },
    // ... 共30个（dailyTarget）
  ],
  "completed": 15,
  "remaining": 15
}
```

---

### 7. 标记单词已学

**端点**：`POST /user/:userId/plans/:planId/words/:word/learn`

**认证**：必需

**请求体**：
```json
{
  "timestamp": 1718611200
}
```

**响应**：
```json
{
  "success": true,
  "nextWord": "ability"  // 下一个待学单词
}
```

---

## 🔄 FSRS复习系统

### 8. 获取待复习卡牌

**端点**：`GET /user/:userId/cards/due`

**认证**：必需

**参数**：
- `limit` (query, optional)：返回数量，默认100

**响应示例**：
```json
{
  "cards": [
    {
      "id": "card-uuid-1",
      "word": "abandon",
      "phase": "Learning",  // New | Learning | Review | Relearning
      "nextReview": 1718611200,
      "reps": 3,
      "stability": 5.2,
      "difficulty": 6.1
    }
  ],
  "total": 45
}
```

---

### 9. 提交复习评分

**端点**：`POST /user/:userId/cards/:cardId/review`

**认证**：必需

**请求体**：
```json
{
  "rating": "Good",  // Again | Hard | Good | Easy
  "timestamp": 1718611200,
  "timeSpent": 8500  // 毫秒，可选
}
```

**响应**：
```json
{
  "success": true,
  "newState": {
    "nextReview": 1718870400,  // 3天后
    "stability": 7.8,
    "difficulty": 5.9,
    "reps": 4
  },
  "nextCard": "card-uuid-2"  // 下一张待复习卡牌
}
```

---

### 10. 批量同步卡牌状态（离线优化）

**端点**：`POST /user/:userId/cards/sync`

**认证**：必需

**描述**：移动端离线学习后批量上传状态

**请求体**：
```json
{
  "updates": [
    {
      "cardId": "card-uuid-1",
      "rating": "Good",
      "timestamp": 1718611200,
      "clientUpdatedAt": 1718611205
    },
    {
      "cardId": "card-uuid-2",
      "rating": "Easy",
      "timestamp": 1718611300,
      "clientUpdatedAt": 1718611305
    }
  ]
}
```

**响应**：
```json
{
  "synced": 2,
  "conflicts": [],  // 冲突的cardId列表（服务器版本更新）
  "latestStates": {
    "card-uuid-1": {
      "nextReview": 1718870400,
      "stability": 7.8
    }
  }
}
```

---

## 🤖 AI内容生成

### 11. 生成单词卡片内容

**端点**：`POST /ai/generate-card`

**认证**：必需（或使用API Key）

**描述**：调用LLM生成助记法、词源分析、例句

**请求体**：
```json
{
  "word": "abandon",
  "context": {
    "chineseTranslation": "v. 放弃；抛弃",
    "englishDefinitions": ["to leave somebody..."]
  },
  "userId": "user-uuid-1",  // 可选，用于用户级缓存
  "useCache": true  // 是否使用缓存（默认true）
}
```

**响应示例**：
```json
{
  "word": "abandon",
  "content": {
    "mnemonics": [
      {
        "type": "visual",
        "content": "a-ban-don：一个（a）禁令（ban）被捐赠（don）= 放弃原有禁令"
      }
    ],
    "etymology": {
      "origin": "来自古法语 'à bandon'（自由支配），后演变为'放弃控制'"
    },
    "examples": [
      {
        "text": "The project was abandoned due to lack of funding.",
        "context": "商业场景"
      }
    ],
    "tips": "注意与 desert（抛弃）的区别：abandon强调主动放弃，desert强调违背责任"
  },
  "model": "gpt-4",
  "cached": false,
  "generatedAt": 1718611200
}
```

---

### 12. 批量预生成AI内容（后台任务）

**端点**：`POST /ai/batch-generate`

**认证**：必需

**描述**：创建学习计划时异步生成前N个单词的AI内容

**请求体**：
```json
{
  "planId": "plan-uuid-1",
  "words": ["abandon", "ability", "about"],
  "priority": "low"  // low | normal | high
}
```

**响应**：
```json
{
  "taskId": "task-uuid-1",
  "status": "queued",
  "estimatedTime": 120  // 秒
}
```

**查询任务状态**：
```http
GET /ai/batch-generate/:taskId
```

**响应**：
```json
{
  "taskId": "task-uuid-1",
  "status": "processing",  // queued | processing | completed | failed
  "progress": {
    "total": 100,
    "completed": 35,
    "failed": 2
  }
}
```

---

## 📊 学习统计

### 13. 获取学习统计

**端点**：`GET /user/:userId/stats`

**认证**：必需

**参数**：
- `period` (query, optional)：统计周期，可选 `day|week|month|all`，默认 `week`

**响应示例**：
```json
{
  "period": "week",
  "totalCards": 520,
  "dueCards": 45,
  "learnedToday": 30,
  "reviewedToday": 25,
  "streak": 15,  // 连续学习天数
  "retention": {
    "rate": 0.87,  // 记忆保持率（87%）
    "trend": "up"  // up | down | stable
  },
  "dailyActivity": [
    {
      "date": "2026-06-17",
      "learned": 30,
      "reviewed": 25,
      "timeSpent": 1800  // 秒
    }
  ],
  "weakWords": [
    {
      "word": "abnormal",
      "forgotTimes": 5,
      "lastReview": 1718611200
    }
  ]
}
```

---

### 14. 学习热力图数据

**端点**：`GET /user/:userId/heatmap`

**认证**：必需

**参数**：
- `year` (query, optional)：年份，默认当前年

**响应示例**：
```json
{
  "year": 2026,
  "data": {
    "2026-01-01": 50,  // 当天学习单词数
    "2026-01-02": 30,
    "2026-01-03": 0,
    // ...
  },
  "maxValue": 120,
  "totalDays": 180
}
```

---

## 📦 词典数据分发

### 15. 获取词典清单

**端点**：`GET /dict/manifest`

**描述**：获取词典数据分片清单（移动端按需下载）

**响应示例**：
```json
{
  "version": "2026.06",
  "shards": [
    {
      "name": "ecdict_a",
      "url": "https://github.com/user/moontranslator-data/releases/download/v2026.06/ecdict_a.json.gz",
      "size": 4521000,  // 字节
      "md5": "a1b2c3d4...",
      "words": ["abandon", "ability", ...]
    },
    {
      "name": "ecdict_b",
      "url": "https://...",
      "size": 3890000,
      "md5": "e5f6g7h8..."
    }
  ],
  "total": 26  // A-Z共26个分片
}
```

---

### 16. 下载词典分片

**端点**：`GET /dict/shard/:name`

**描述**：代理GitHub下载，提供进度和缓存

**参数**：
- `name` (path, required)：分片名称，如 `ecdict_a`

**响应**：
- `Content-Type: application/json`
- 直接返回解压后的JSON数据（Workers自动解压gzip）

---

## 🔐 用户认证

### 17. 微信登录

**端点**：`POST /auth/wechat`

**描述**：微信小程序登录

**请求体**：
```json
{
  "code": "wx-login-code",
  "encryptedData": "...",
  "iv": "..."
}
```

**响应**：
```json
{
  "token": "jwt-token",
  "userId": "user-uuid-1",
  "expiresIn": 7200
}
```

---

### 18. 设备注册（匿名模式）

**端点**：`POST /auth/device`

**描述**：桌面端/移动端匿名使用

**请求体**：
```json
{
  "deviceId": "uuid-v4",
  "platform": "windows",  // windows | macos | linux | ios | android
  "version": "1.0.0"
}
```

**响应**：
```json
{
  "userId": "anon-user-uuid",
  "token": "jwt-token"
}
```

---

## 📤 数据导出/导入

### 19. 导出用户数据

**端点**：`GET /user/:userId/export`

**认证**：必需

**描述**：导出所有学习数据（GDPR合规）

**响应**：
```json
{
  "user": {
    "id": "user-uuid-1",
    "createdAt": 1718611200
  },
  "plans": [...],
  "cards": [...],
  "reviews": [...],
  "exportedAt": 1718611200
}
```

---

### 20. 导入用户数据

**端点**：`POST /user/:userId/import`

**认证**：必需

**请求体**：导出的JSON数据

**响应**：
```json
{
  "imported": {
    "plans": 3,
    "cards": 520,
    "reviews": 1200
  },
  "skipped": 15,  // 重复数据
  "errors": []
}
```

---

## ⚠️ 错误代码

| 错误码                | HTTP状态 | 说明              |
|----------------------|---------|------------------|
| `WORD_NOT_FOUND`     | 404     | 单词未找到        |
| `INVALID_RATING`     | 400     | 无效的评分        |
| `USER_NOT_FOUND`     | 404     | 用户不存在        |
| `PLAN_NOT_FOUND`     | 404     | 学习计划不存在    |
| `CARD_NOT_FOUND`     | 404     | 卡牌不存在        |
| `UNAUTHORIZED`       | 401     | 未认证            |
| `RATE_LIMIT_EXCEEDED`| 429     | 请求频率超限      |
| `AI_GENERATION_FAILED`| 500    | AI生成失败        |
| `DATABASE_ERROR`     | 500     | 数据库错误        |

**错误响应格式**：
```json
{
  "error": "Human-readable error message",
  "code": "ERROR_CODE",
  "details": {
    "field": "rating",
    "received": "invalid_value"
  }
}
```

---

## 🚀 性能优化建议

### 客户端
1. **批量请求**：使用 `/word/batch` 减少网络往返
2. **增量同步**：只上传 `dirty=1` 的记录
3. **缓存策略**：
   - 词典数据：永久缓存（根据manifest版本失效）
   - AI内容：缓存7天
   - 用户数据：实时

### 服务端（Cloudflare Workers）
1. **KV缓存**：
   - 热词查询结果：TTL 1天
   - AI生成内容：TTL 7天
   - 词典清单：TTL 1小时

2. **边缘计算**：Workers自动在全球部署，延迟 <50ms

3. **限流策略**：
   - 匿名用户：100 req/min
   - 认证用户：500 req/min
   - AI生成：10 req/min

---

## 📝 开发测试

### Mock数据端点（开发环境）
```http
GET /dev/mock/word/:word
GET /dev/mock/user/:userId/cards/due
```

### Postman Collection
提供完整的API测试集合：`docs/moontranslator-api.postman_collection.json`

---

**维护者**：开发团队  
**反馈渠道**：GitHub Issues  
**更新频率**：每次重大版本发布
