# MoonTranslator Cloudflare API

Cloudflare Workers API for MoonTranslator cloud services.

## 功能特性

- 📚 **词典查询** - 聚合 ECDICT 数据，支持联想搜索和批量查询
- 📖 **学习数据** - 学习计划管理、卡牌状态同步
- 🔄 **数据同步** - 离线优先，支持多设备同步
- 🤖 **AI 代理** - 代理 LLM 请求，缓存高频词 AI 内容

## 技术栈

- **Runtime**: Cloudflare Workers
- **Framework**: Hono
- **Database**: Cloudflare D1 (SQLite)
- **Cache**: Cloudflare KV
- **Language**: TypeScript

## 部署步骤

### 1. 安装依赖

```bash
cd cloudflare-api
npm install
```

### 2. 创建 D1 数据库

```bash
# 创建数据库
wrangler d1 create moontranslator-db

# 记录返回的 database_id，更新到 wrangler.toml
```

### 3. 创建 KV 命名空间

```bash
# 创建 KV
wrangler kv namespace create CACHE

# 记录返回的 id，更新到 wrangler.toml
```

### 4. 初始化数据库

```bash
# 执行 schema
wrangler d1 execute moontranslator-db --file=src/db/schema.sql
```

### 5. 配置环境变量

编辑 `wrangler.toml`：

```toml
[vars]
GITHUB_DATA_REPO = "your-username/moontranslator-data"
GITHUB_TOKEN = "YOUR_GITHUB_TOKEN"  # 私有仓库需要
```

### 6. 本地开发

```bash
npm run dev
```

### 7. 部署

```bash
npm run deploy
```

## API 端点

### 词典查询

- `GET /api/v1/dict/suggest?q={query}` - 搜索建议
- `GET /api/v1/dict/lookup/{word}` - 单词详情
- `POST /api/v1/dict/batch` - 批量查询

### 学习数据

- `GET /api/v1/learning/plans/{userId}` - 获取学习计划
- `POST /api/v1/learning/plans` - 创建学习计划
- `GET /api/v1/learning/cards/due/{userId}` - 获取待复习卡牌
- `POST /api/v1/learning/review` - 提交复习结果

### 数据同步

- `GET /api/v1/sync/pull/{userId}?since={timestamp}` - 拉取更新
- `POST /api/v1/sync/push/{userId}` - 推送更新
- `GET /api/v1/sync/status/{userId}` - 同步状态

### AI 代理

- `GET /api/v1/ai/content/{word}` - 获取 AI 内容（缓存）
- `POST /api/v1/ai/content` - 保存 AI 内容
- `POST /api/v1/ai/content/batch` - 批量获取

## 数据同步策略

### 离线优先

1. 所有操作先写本地 SQLite
2. 标记 `dirty = 1`
3. 后台每 5 分钟同步一次

### 冲突解决

- Last-Write-Wins (LWW) 策略
- 基于 `updated_at` 时间戳
- 服务端时间作为权威时间源

### 同步流程

```
客户端                    服务端
  │                         │
  │  GET /sync/pull?since=  │
  │ ───────────────────────>│
  │                         │
  │  { cards, plans, events }│
  │ <───────────────────────│
  │                         │
  │  POST /sync/push        │
  │ ───────────────────────>│
  │                         │
  │  { syncedCount, errors }│
  │ <───────────────────────│
  │                         │
```

## GitHub 数据源

词典数据托管在 GitHub 仓库 `moontranslator-data`：

```
moontranslator-data/
├── ecdict/
│   ├── manifest.json
│   ├── ecdict_a.json.gz
│   ├── ecdict_b.json.gz
│   └── ...
└── ai-cache/
    └── common_1000.json
```

## 成本估算

### Cloudflare 免费额度

- Workers: 100,000 请求/天
- D1: 5M 读取/天, 100K 写入/天
- KV: 100K 读取/天, 1K 写入/天

### 预计用量

- 词典查询: ~10K/天（免费额度内）
- 数据同步: ~5K/天（免费额度内）
- AI 代理: 按需（建议使用免费 LLM API）

## 相关文档

- [API 规范](../docs/API_SPEC.md)
- [数据库设计](../docs/DATABASE.md)
- [开发路线图](../docs/ROADMAP.md)
