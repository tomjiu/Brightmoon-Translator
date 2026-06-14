# Vocabulary Learning System - 词汇学习系统

MoonTranslator 的智能词汇学习模块，基于 Event Sourcing + FSRS + AI。

## ✨ 核心特性

### 🎯 智能学习系统
- **FSRS-4.5 算法**: 科学的间隔重复调度
- **学习阶段管理**: New → Learning → Review → Mastered
- **个性化复习**: 基于记忆曲线自动调度

### 🤖 AI 内容生成
- **词源分析**: 词根拆解、历史演变
- **多样助记法**: 词源、场景、谐音、视觉、拆分
- **个性化例句**: 根据难度和场景定制
- **智能优化**: 自动检测学习困难并优化内容

### 📊 完整数据追踪
- **Event Sourcing**: 完整学习历史，可回溯任意时刻
- **版本管理**: 支持内容版本控制和回退
- **统计分析**: 学习时长、正确率、进度可视化

### 📚 丰富词库资源
- **核心词库**: 15,000 高频词（按词频排序）
- **词根数据**: 70,000+ 词根拆解（MorphoLex）
- **权威标注**: Collins 星级、Oxford 3000 标签
- **考试分类**: CET4/6, IELTS, TOEFL, GRE

## 🏗️ 技术架构

### 后端 (Rust)
```
Domain Layer (领域层)
├── Event Sourcing      - 事件流存储
├── FSRS Engine         - 间隔重复算法
├── State Machine       - 学习状态管理
└── Patch System        - 内容优化系统

Skills Layer (技能层)
├── Dictionary Skill    - 词典查询 (ECDICT)
├── Morphology Skill    - 词根拆解 (MorphoLex)
├── Generate Card       - AI 内容生成
└── Optimize Card       - AI 智能优化

Infrastructure (基础设施)
├── Event Store         - SQLite 事件存储
├── Data Initializer    - 数据导入工具
└── LLM Provider        - OpenAI 兼容接口
```

### 前端 (React + TypeScript)
```
Components              - React UI 组件
├── CoreVocabularyList  - 词库列表
├── CardDetail          - 卡牌详情
├── ReviewCard          - 复习界面
└── LearningStatsPanel  - 统计面板

State Management        - 状态管理
├── React Query         - 服务端状态 + 缓存
└── Zustand             - 客户端状态 + 持久化

Services                - API 封装
└── vocabulary.ts       - Tauri API 调用
```

## 🚀 快速开始

### 初始化数据库

```bash
cd src-tauri
cargo run --example data_init_demo
```

这将导入：
- 15,000 核心高频词
- 70,000+ 词根数据
- 索引优化

### 配置 LLM

在配置文件中添加 OpenAI 兼容的 API：

```json
{
  "llm": {
    "api_keys": ["sk-..."],
    "base_url": "https://api.openai.com/v1",
    "model": "gpt-4"
  }
}
```

## 📖 使用指南

### 1. 浏览词库

- 查看 15,000 核心高频词
- 按词频排序
- Collins 星级、Oxford 3000 标签
- 支持搜索

### 2. 学习新词

1. 选择单词创建卡牌
2. AI 自动生成：
   - 词源分析
   - 助记法（5种类型）
   - 个性化例句
   - 场景对话
3. 开始学习

### 3. 复习卡牌

1. 查看待复习列表
2. 问答模式复习
3. 4级评分（Again/Hard/Good/Easy）
4. 自动更新下次复习时间

### 4. 智能优化

系统自动检测：
- 低分（< 3分）
- 频繁遗忘（≥3次）
- 错误记录

触发 AI 优化：
- 分析问题原因
- 生成优化 Patch
- 改进助记法和例句

## 🎯 核心概念

### Event Sourcing
所有操作记录为事件流：
```rust
CardEvent::WordImported        // 导入单词
CardEvent::AiContentGenerated  // AI 生成内容
CardEvent::FsrsUpdated         // FSRS 更新
CardEvent::PatchApplied        // Patch 应用
```

### FSRS-4.5 算法
科学的间隔重复：
- **Stability**: 记忆稳定性（天数）
- **Difficulty**: 难度系数
- **Rating**: 4级评分
- **Scheduling**: 自动调度

### Patch System
安全的内容优化：
```rust
CardPatch {
    target_field: "mnemonic",
    operation: Replace,
    proposed_value: "...",
    confidence: 0.9,
}
```

## 📊 统计数据

### 代码统计
```
Rust 后端:      5,937 行
TypeScript 前端: 1,197 行
----------------------
总计:           7,134 行
```

### 测试覆盖
- ✅ 单元测试: 26 个
- ✅ 集成测试: 3 个
- ✅ 演示程序: 3 个

### 性能指标
- 词库查询: < 1ms
- 事件重建: < 10ms (100 events)
- AI 生成: 1-5s (依赖 LLM)

## 🛠️ API 端点

### Tauri Commands

```rust
// 核心词库
get_core_vocabulary(offset: i64, limit: i64) -> Vec<CoreVocabEntry>
search_core_vocabulary(query: String, limit: i64) -> Vec<CoreVocabEntry>

// 卡牌管理
create_card(word: String) -> String
get_card(card_id: String) -> WordCard
get_due_cards() -> Vec<CardInfo>

// AI 功能
generate_card_content(card_id: String) -> AiContent
submit_review(card_id: String, rating: Rating) -> ()

// 统计
get_learning_stats() -> LearningStats
```

## 📁 项目结构

```
src-tauri/
├── src/
│   ├── domain/                 # 领域层
│   │   ├── event.rs            # 事件定义
│   │   ├── card.rs             # 卡牌实体
│   │   ├── fsrs_engine.rs      # FSRS 算法
│   │   ├── state_machine.rs    # 状态机
│   │   ├── patch_validator.rs  # Patch 验证
│   │   └── patch_applicator.rs # Patch 应用
│   │
│   ├── infrastructure/         # 基础设施层
│   │   ├── event_store.rs      # 事件存储
│   │   └── data_init.rs        # 数据初始化
│   │
│   ├── skills/                 # 技能层
│   │   ├── mod.rs              # Skill 注册
│   │   ├── dictionary.rs       # 词典查询
│   │   ├── morphology.rs       # 词根拆解
│   │   ├── llm_provider.rs     # LLM 接口
│   │   ├── generate_card.rs    # AI 生成
│   │   └── optimize_card.rs    # AI 优化
│   │
│   └── commands/               # Tauri Commands
│       └── vocabulary_cmd.rs   # 词汇 API
│
├── examples/                   # 示例程序
│   ├── event_store_demo.rs
│   ├── llm_skills_demo.rs
│   └── data_init_demo.rs
│
├── migrations/                 # 数据库迁移
│   └── 001_initial_schema.sql
│
└── tests/                      # 测试
    └── integration_test.rs

src/
├── components/vocabulary/      # React 组件
│   ├── CoreVocabularyList.tsx
│   ├── CardDetail.tsx
│   ├── ReviewCard.tsx
│   └── LearningStatsPanel.tsx
│
├── hooks/                      # React Hooks
│   └── useVocabulary.ts
│
├── services/                   # API 服务
│   └── vocabulary.ts
│
└── stores/                     # 状态管理
    └── vocabularyStore.ts
```

## 🔬 测试

### 运行单元测试
```bash
cd src-tauri
cargo test
```

### 运行集成测试
```bash
cd src-tauri
cargo test --test integration_test
```

### 运行示例程序
```bash
cd src-tauri
cargo run --example event_store_demo
cargo run --example llm_skills_demo
cargo run --example data_init_demo
```

## 🙏 致谢

- [ECDICT](https://github.com/skywind3000/ECDICT) - 开源英汉词典
- [MorphoLex](https://github.com/hugomailhot/MorphoLex-en) - 英语词根数据
- [FSRS](https://github.com/open-spaced-repetition/fsrs-rs) - 间隔重复算法

---

**功能状态**: ✅ 核心功能完成 (80%)

**最后更新**: 2026-06-14
