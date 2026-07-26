# Moon Translator - 多平台开发路线图

**项目定位**：AI驱动的多源词典 + 智能学习系统  
**目标平台**：桌面App (Tauri) + 移动App + 微信小程序  
**云端方案**：Cloudflare Workers / Pages + GitHub Repo 作为数据存储  
**更新时间**：2026-06-17

---

## 📋 架构规划

### 多端架构设计

```
┌─────────────────┬─────────────────┬─────────────────┐
│   桌面端 (Tauri) │   移动端 (RN)   │   小程序 (微信)  │
└────────┬────────┴────────┬────────┴────────┬────────┘
         │                 │                 │
         └─────────────────┼─────────────────┘
                           ↓
                 ┌──────────────────┐
                 │  统一 REST API   │
                 │  (Cloudflare)    │
                 └─────────┬────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ↓                 ↓                 ↓
   GitHub Repo       Cloudflare KV    Cloudflare D1
   (词典数据)         (用户偏好)         (学习记录)
```

### 数据分层策略

1. **静态词典数据**（只读，体积大）
   - ECDICT 词库：GitHub Release 托管（压缩后 ~50MB）
   - Oxford/GPT4-Dict：拆分为多个JSON文件，按字母索引
   - 移动端/小程序：按需下载分片

2. **用户学习数据**（读写，云同步）
   - 学习计划、卡牌状态：Cloudflare D1 (SQLite)
   - FSRS复习记录：需要跨设备同步
   - 离线优先 + 后台同步策略

3. **AI生成内容缓存**（混合）
   - 高频词AI内容：预生成并托管在CDN
   - 低频词：实时生成 + 云端缓存（KV存储）

---

## 🎯 开发阶段规划

### **阶段 0：架构重构准备**（1-2周）⚠️ 当前阶段

**目标**：使现有桌面端代码可被多端复用

#### 0.1 API抽象层
- [ ] 创建 `src-tauri/src/api/` 模块，封装所有业务逻辑
  ```rust
  // api/dictionary.rs - 词典查询API（无Tauri依赖）
  pub async fn lookup_word(word: &str, sources: Vec<Source>) -> Result<WordEntry>
  
  // api/learning.rs - 学习系统API
  pub async fn get_due_cards(user_id: &str) -> Result<Vec<Card>>
  pub async fn submit_review(card_id: &str, rating: Rating) -> Result<FsrsState>
  ```

- [ ] 现有 `commands/` 改为薄封装层，调用 `api/` 模块
  ```rust
  #[tauri::command]
  async fn lookup_word_multi_source(word: String, state: State<'_, AppState>) 
    -> Result<ComprehensiveEntry, String> 
  {
      crate::api::dictionary::lookup_word(&word, vec![Source::ECDICT, Source::Youdao])
          .await
          .map_err(|e| e.to_string())
  }
  ```

#### 0.2 云端API接口设计
创建 `docs/API_SPEC.md`，定义RESTful接口（Cloudflare Workers实现）

```yaml
# 核心端点
GET  /api/v1/word/:word              # 查词（聚合多源）
GET  /api/v1/user/:uid/cards/due     # 待复习卡牌
POST /api/v1/user/:uid/review        # 提交复习
POST /api/v1/user/:uid/plan          # 创建学习计划
GET  /api/v1/dict/suggest?q=:query   # 搜索建议
```

#### 0.3 数据库迁移方案
- [ ] 设计云端Schema（Cloudflare D1）
  ```sql
  -- 用户表（多设备关联）
  CREATE TABLE users (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
  );
  
  -- 学习计划（云端同步）
  CREATE TABLE learning_plans (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    name TEXT,
    words_json TEXT, -- 单词列表JSON
    daily_target INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
  );
  
  -- 卡牌状态（核心同步数据）
  CREATE TABLE card_states (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    word TEXT NOT NULL,
    fsrs_state TEXT NOT NULL, -- JSON格式
    last_review INTEGER,
    next_review INTEGER,
    synced_at INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
  );
  ```

- [ ] 桌面端保留本地SQLite，增加同步字段（`synced_at`, `dirty`）

---

### **阶段 1：桌面端功能完善**（2-3周）

#### 1.1 复习系统完整实现
- [x] 复习模式UI（4级评分：Again/Hard/Good/Easy）
  - 文件：`src/pages/VocabularyReview.tsx`
  - 调用：`get_due_cards()` + `submit_review()`
  - 显示下次复习时间、累计复习数

- [x] 复习统计Dashboard
  - 今日复习数 / 待复习数
  - 7天/30天复习热力图
  - 记忆保持率曲线
  - 薄弱词汇分析
  - 文件：`src/components/vocabulary/LearningStatsDashboard.tsx`

#### 1.2 AI内容批量预生成
- [x] 后台任务：创建计划时异步生成前100词AI内容
  - 文件：`src-tauri/src/tasks/batch_generation.rs`
  - 并发控制（Semaphore max 3）
  - 进度追踪（completed/failed/current_word）

- [x] 生成进度通知（Tauri Event）
  - 文件：`src/components/vocabulary/AIGenerationProgress.tsx`
  - 实时进度条 + 完成统计
  - 右下角浮动通知

#### 1.2.1 学习提醒和通知系统
- [x] 桌面通知（跨平台 Windows/macOS/Linux）
- [x] 每日学习提醒（可配置时间）
- [x] 待复习卡牌提醒（可配置阈值）
- [x] 学习里程碑庆祝（3/7/14/30/60/100/365天）
- [x] 学习计划进度提醒
  - 文件：`src-tauri/src/commands/notification_cmd.rs`
  - 前端设置：`src/components/vocabulary/NotificationManager.tsx`

#### 1.2.2 单词详情增强
- [x] 学习历史时间线（导入/复习/AI生成事件）
- [x] FSRS 参数变化曲线（难度/稳定性可视化）
- [x] 手动编辑 AI 内容
- [x] 相关词汇推荐（同根词前缀匹配）
- [x] 语料库例句
- [x] 词根词缀分析（15前缀 + 10后缀）
  - 文件：`src-tauri/src/commands/word_detail_cmd.rs`
  - 前端：`src/components/vocabulary/WordDetailModal.tsx`

#### 1.2.3 多样化学习模式
- [x] 选择题模式（4选1，测试理解）
- [x] 拼写模式（根据释义拼写单词）
- [x] 填空模式（选词填空）
- [x] 快速复习模式（卡片翻转 + 左右滑动评分）
- [x] 键盘快捷键（1-4/空格/回车/方向键）
  - 文件：`src-tauri/src/commands/learning_mode_cmd.rs`
  - 前端：`src/components/vocabulary/modes/` (4个组件)

#### 1.2.4 学习数据导入导出
- [x] JSON 全量导出（卡牌 + FSRS状态 + AI内容 + 活动数据）
- [x] Anki TSV 导出（HTML释义 + 标签分类）
- [x] CSV 通用导出（Excel 兼容）
- [x] JSON 备份恢复（自动去重）
- [x] CSV/TSV 单词列表导入（兼容 Quizlet/扇贝）
- [x] 自动备份到指定目录
  - 文件：`src-tauri/src/commands/data_io_cmd.rs`
  - 前端：`src/pages/DataIO.tsx`

#### 1.2.5 FSRS 算法分析与优化
- [x] FSRS 分析报告（参数/保持率/平均难度/稳定性）
- [x] 遗忘曲线可视化（SVG 图表 + 80%基准线）
- [x] 未来30天复习量预测
- [x] 最佳学习时段分析（按小时正确率）
- [x] 卡牌难度分布（10区间直方图）
- [x] 智能优化建议
  - 文件：`src-tauri/src/commands/fsrs_optimization_cmd.rs`
  - 前端：`src/pages/FsrsOptimization.tsx`

#### 1.3 离线词典优化
- [x] ECDICT 数据压缩（移除低频词，精简释义字段）
  - 支持按词频范围导出（3000/5000/8000/15000）
  - 文件：`src-tauri/src/commands/dict_optimize_cmd.rs`
  - 前端：`src/pages/DictOptimization.tsx`

- [x] 词典数据分片（按首字母拆分为 26 个分片）
  - 生成 `ecdict_a.db` ~ `ecdict_z.db`
  - 自动生成 `manifest.json` 清单文件
  - 适合移动端按需下载和 GitHub Release 托管

---

### **阶段 2：云端基础设施**（2-3周）

#### 2.1 Cloudflare Workers API
- [x] 创建 `cloudflare-api/` 目录（独立项目）
  - 技术栈：Hono + D1 + KV
  - 文件：`cloudflare-api/`

- [x] 词典查询端点（聚合 ECDICT）
  - 搜索建议（联想词）
  - 单词详情查询
  - 批量查询（最多 50 词）
  - KV 缓存（建议 1 小时，详情 24 小时）

- [x] 用户学习数据 CRUD
  - 学习计划管理（创建/查询）
  - 待复习卡牌获取
  - 复习结果提交（简化版 FSRS）

- [x] 数据同步 API
  - 拉取更新（增量同步）
  - 推送更新（Last-Write-Wins）
  - 同步状态查询

- [x] AI 内容代理
  - KV 缓存高频词 AI 内容（1 年 TTL）
  - 批量获取接口
  - 内容保存接口

#### 2.2 GitHub作为静态数据源
- [x] 创建 `github-data/` 仓库结构
  ```
  github-data/
  ├── ecdict/               # 词典数据
  │   ├── manifest.json     # 版本 + 分片清单
  │   ├── ecdict_a.json.gz  # 压缩后 <5MB
  │   └── ...
  ├── ai-cache/             # AI 内容缓存
  │   └── common_1000.json  # 高频 1000 词
  └── .github/workflows/
      └── build.yml         # 自动构建 workflow
  ```

- [x] 数据导出工具
  - `export_for_github` - 导出词典到 GitHub 格式（JSON + GZ）
  - `export_ai_cache_for_github` - 导出 AI 内容缓存
  - 文件：`src-tauri/src/commands/github_export_cmd.rs`

- [x] GitHub Actions 自动构建
  - 自动压缩 JSON 文件为 GZ
  - 生成 SHA256 校验和
  - 自动创建 Release

- [x] Cloudflare Workers 集成
  - `DictLoader` 服务从 GitHub 加载数据
  - KV 缓存（建议 1 小时，详情 24 小时，分片 7 天）
  - 文件：`cloudflare-api/src/services/dict-loader.ts`

#### 2.3 数据同步策略
- [ ] **离线优先**：所有操作先写本地，成功后标记 `dirty=1`
- [ ] **后台同步**：
  ```typescript
  // 每5分钟同步一次
  setInterval(async () => {
      const dirtyRecords = await db.getDirtyRecords();
      await fetch('/api/v1/sync', {
          method: 'POST',
          body: JSON.stringify(dirtyRecords)
      });
  }, 300000);
  ```

- [ ] **冲突解决**：Last-Write-Wins（基于 `updated_at` 时间戳）

---

### **阶段 3：移动端App开发**（4-5周）

#### 3.1 技术栈选型
**推荐方案**：React Native + Expo

**理由**：
- 复用现有React/TypeScript前端代码（80%+）
- Expo提供完善的原生模块（SQLite、文件系统）
- 支持OTA更新（绕过应用商店审核）

**替代方案**：Flutter（如需更高性能）

#### 3.2 移动端架构调整
```
mobile-app/
├── src/
│   ├── api/              # API客户端（调用Cloudflare Workers）
│   ├── db/               # 本地SQLite（复用桌面端Schema）
│   ├── screens/          # 页面（复用桌面端组件）
│   ├── components/       # UI组件（Tauri组件需改为RN组件）
│   └── sync/             # 同步逻辑
├── assets/
│   └── dict-cache/       # 词典分片缓存目录
└── app.json
```

#### 3.3 移动端特性开发
- [ ] **词典分片按需下载**
  - 首次安装只下载 `manifest.json` + 常用3000词
  - 查询时按需下载对应字母分片（缓存到本地）
  - 显示下载进度

- [ ] **离线学习模式**
  - 所有操作优先写本地SQLite
  - 后台静默同步到云端

- [ ] **推送通知**
  - 每日学习提醒（Expo Notifications）
  - 复习提醒（基于FSRS下次复习时间）

- [ ] **语音朗读**
  - 集成TTS（iOS: AVSpeechSynthesizer，Android: TextToSpeech）
  - 离线发音（避免依赖网络）

---

### **阶段 4：微信小程序开发**（3-4周）

#### 4.1 小程序架构
```
miniprogram/
├── pages/
│   ├── index/            # 首页（词典查询）
│   ├── learning/         # 学习页面
│   ├── review/           # 复习页面
│   └── stats/            # 统计页面
├── api/
│   └── request.ts        # wx.request 封装（调用Cloudflare API）
├── utils/
│   └── storage.ts        # wx.setStorageSync 封装
└── app.json
```

#### 4.2 小程序限制与对策
**限制1：包体积 ≤ 20MB（主包 ≤ 2MB）**
- **对策**：
  - 词典数据全部走云端API（不打包）
  - 图片资源使用CDN（Cloudflare Images）
  - 代码分包加载

**限制2：不支持SQLite**
- **对策**：
  - 使用 `wx.setStorageSync`（上限10MB）存储常用词缓存
  - 学习数据全部云端存储（D1数据库）

**限制3：网络请求需域名备案**
- **对策**：
  - Cloudflare Workers 绑定已备案域名（如 `api.yourdomain.com`）
  - 或使用代理服务器（国内云服务商）

#### 4.3 小程序特色功能
- [ ] **微信分享**
  - 分享学习成果卡片（今日学习词数、连续天数）
  - 分享学习计划（好友可一键导入）

- [ ] **小组PK模式**
  - 创建学习小组（最多20人）
  - 实时排行榜（当周学习词数）

- [ ] **小程序码打卡**
  - 每日学习完成后生成打卡海报
  - 一键分享到朋友圈（网页版）

---

### **阶段 5：跨平台协同优化**（2周）

#### 5.1 数据同步测试
- [ ] 场景1：桌面端创建计划 → 移动端立即可见
- [ ] 场景2：小程序学习 → 桌面端统计实时更新
- [ ] 场景3：离线学习 → 联网后自动同步

#### 5.2 性能优化
- [ ] **词典查询**：
  - Cloudflare Workers 缓存热词（TTL 1天）
  - GitHub静态资源启用 CDN
  
- [ ] **AI生成**：
  - 高频1000词预生成（托管在CDN）
  - 低频词实时生成（缓存到KV，TTL 7天）

#### 5.3 安全加固
- [ ] 用户认证：支持微信登录 + 邮箱注册
- [ ] API限流：Cloudflare Workers 限制每IP 100请求/分钟
- [ ] 敏感数据加密：用户LLM API Key使用AES加密存储

---

## 🛠️ 技术栈对比

| 功能模块       | 桌面端 (Tauri)       | 移动端 (React Native) | 小程序             |
|---------------|---------------------|----------------------|-------------------|
| UI框架        | React + TailwindCSS | React Native Paper   | WeUI / Vant Weapp |
| 数据库        | SQLite (sqlx)       | SQLite (expo-sqlite) | wx.storage API    |
| 网络请求      | reqwest (Rust)      | fetch / axios        | wx.request        |
| 状态管理      | Zustand             | Zustand / Redux      | 全局变量 / Pinia  |
| AI调用        | 直接HTTP (Rust)     | 通过Cloudflare代理   | 通过Cloudflare代理|
| 词典数据      | 本地 .db 文件       | 分片按需下载          | 云端API          |

---

## 📦 发布计划

### 桌面端
- **Windows**：NSIS安装包 + 便携版
- **macOS**：DMG + 自动更新（Tauri Updater）
- **Linux**：AppImage + deb包

### 移动端
- **iOS**：App Store（需苹果开发者账号 $99/年）
- **Android**：Google Play + 国内应用商店（小米、华为、OPPO等）

### 小程序
- **微信小程序**：需企业认证（300元/年）
- **支付宝小程序**：复用代码，适配支付宝API

---

## 🔄 持续集成 (CI/CD)

### GitHub Actions工作流
```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build-desktop:
    # 构建桌面端（Windows/Mac/Linux）
  
  build-mobile:
    # 构建移动端APK/IPA
  
  deploy-api:
    # 部署Cloudflare Workers
  
  update-data:
    # 更新词典数据到GitHub Release
```

---

## 📊 里程碑检查点

- [ ] **M1（2周后）**：API抽象层完成，桌面端复习系统可用
- [ ] **M2（1月后）**：Cloudflare Workers API上线，数据同步测试通过
- [ ] **M3（2月后）**：移动端Beta版发布，支持离线学习
- [ ] **M4（3月后）**：小程序提审通过，三端数据互通
- [ ] **M5（4月后）**：正式版1.0发布，开启用户推广

---

## 🎯 优先级标注

- 🔥 **P0（必须）**：复习系统、云端API、数据同步
- ⚡ **P1（重要）**：AI内容预生成、移动端App
- 💡 **P2（可选）**：小程序、社交功能、高级统计

---

## 📝 开发注意事项

### 架构原则
1. **API优先**：所有业务逻辑封装为API，可被多端调用
2. **离线优先**：网络断开时功能不受影响
3. **增量更新**：词典数据按需下载，减少初始体积
4. **数据安全**：用户数据加密存储，支持导出备份

### 代码组织
```
moontranslator/
├── desktop/          # 桌面端（Tauri）
│   ├── src-tauri/    # Rust后端
│   └── src/          # React前端
├── mobile/           # 移动端（React Native）
├── miniprogram/      # 微信小程序
├── cloudflare-api/   # Cloudflare Workers API
├── data-repo/        # 词典数据仓库（独立Git仓库）
└── docs/             # 文档
    ├── API_SPEC.md   # API接口文档
    ├── DATABASE.md   # 数据库设计
    └── ROADMAP.md    # 本文档
```

### 版本管理
- 桌面端：v1.x.x
- 移动端：v2.x.x
- 小程序：v3.x.x
- API版本：/api/v1/ （向后兼容）

---

**最后更新**：2026-06-17  
**负责人**：待定  
**下次Review**：2周后（2026-07-01）
