# 项目进度总结 - 2026-06-14

## ✅ 今日完成

### 1. 架构设计确定

经过深度讨论，确定了**完整的 AI 学习系统架构**：

#### 核心设计原则
- ✅ **Event Sourcing**: 事件流是唯一真相，卡牌是派生视图
- ✅ **AI 生成 Patch**: AI 只提议变更，不直接修改卡牌
- ✅ **State Machine**: Learning Workflow 驱动，AI 是 Worker
- ✅ **Skill > MCP**: Skill 是稳定 API，MCP 是可选 Provider

#### 技术栈锁定
```
LLM Client:       rust-genai (多提供商统一接口)
Structured Output: schemars + JsonSpec
FSRS Algorithm:   fsrs (官方 Rust 实现)
MCP Protocol:     modelcontextprotocol/rust-sdk
Database:         sqlx + SQLite (Event Store + FTS5)
State Machine:    自实现 (enum + match)
Event Bus:        tokio::sync::broadcast
```

#### 数据库 Schema 设计
- `card_events` - 事件流（唯一真相）
- `cards` - 快照（可从事件重建）
- `card_patches` - 版本控制（支持回退）
- `user_profile` / `weak_points` / `review_logs` - 用户数据
- `quiz_errors` / `annotations` - 学习反馈

### 2. 词典资源下载完成

✅ **ECDICT 主词库**
- 文件: `dictionaries/ecdict.db`
- 大小: 812MB
- 收词: 324万词
- 格式: SQLite
- 特性: 含音标/释义/词频/考试标签(toefl/ielts/gre)/词干/时态变形

✅ **GPT4 词根词典**
- 文件: `dictionaries/gpt4-dict/gptwords.json`
- 大小: 17MB
- 收词: 8000核心词
- 格式: JSON
- 特性: 详细词根拆解/助记法/文化背景/例句

### 3. 文档产出

#### LEARNING_SYSTEM_ARCHITECTURE.md
- 完整系统分层架构
- 事件定义（CardEvent 枚举）
- Skill 列表（10+ 内置 Skills）
- 完整学习流程
- 7周开发计划

#### DICTIONARY_RESOURCES.md
- 词典资源对比（5个开源项目）
- License 分析
- 分层数据架构
- 下载脚本
- 导入建议

#### FINAL_RUST_GENAI_SOLUTION.md
- rust-genai 完整使用方案
- 代码示例
- 成本估算

#### RUST_AI_SOLUTION.md
- Rust AI 生态调研
- 框架对比（rig/llm-chain-rs/candle等）

#### BROWSER_EXTENSION_LEARNING_PLAN.md
- 浏览器扩展集成方案
- 3阶段实施计划

#### AI_AGENT_FRAMEWORK_SELECTION.md
- 框架选型分析
- Instructor/LangChain 对比

---

## 🎯 核心架构洞察

### 为什么不是传统 Agent 框架？

**传统做法**:
```
Agent Framework (LangChain/AutoGen)
  ↓
Agent 自主驱动（Tool Calling Loop）
  ↓
直接修改数据库
```

**我们的做法**:
```
Learning State Machine (确定性状态机)
  ↓
AI Coordinator (生成 Patch)
  ↓
Event Sourcing (不可变事件流)
  ↓
Patch Validator (验证后应用)
  ↓
Version History (可回退/重放)
```

### 为什么这样设计？

1. **控制权**: LLM 不应该控制学习流程（复习间隔、进度管理），只负责生成内容
2. **可回退**: 用户可以回到任意历史版本
3. **可重放**: 升级模型后，可以用新模型重新生成所有卡牌
4. **可调试**: 完整事件历史，可追溯每个变更的原因
5. **A/B 测试**: 可以对比不同模型/Prompt 的效果

---

## 📋 下一步计划

### Phase 1: 核心基础设施（2周）

#### Week 1
- [ ] 创建 Event Store 模块
  - `src-tauri/src/domain/event.rs` - CardEvent 定义
  - `src-tauri/src/infrastructure/event_store.rs` - SQLite 实现
  - 数据库迁移脚本

- [ ] 实现 Patch 系统
  - `src-tauri/src/domain/patch.rs` - CardPatch 定义
  - `src-tauri/src/domain/patch_validator.rs` - 验证器
  - 版本控制逻辑

- [ ] WordCard 从事件重放
  - `src-tauri/src/domain/card.rs` - WordCard 结构
  - `apply_event()` 方法
  - `from_events()` 重建

#### Week 2
- [ ] FSRS 集成
  - 添加 `fsrs` crate 依赖
  - `src-tauri/src/domain/fsrs_engine.rs`
  - 复习调度逻辑

- [ ] 数据库完整 schema
  - 执行迁移脚本
  - 索引优化
  - FTS5 全文搜索配置

### Phase 2: Skill + LLM（1周）

- [ ] Skill trait 定义
- [ ] SkillRegistry 实现
- [ ] DictionarySkill（接入 ECDICT）
- [ ] rust-genai Provider 封装
- [ ] GenerateCardSkill（LLM + JSON Schema）

---

## 📊 项目状态

### 已完成
✅ 架构设计  
✅ 技术栈选型  
✅ 词典资源下载  
✅ 开发计划制定  
✅ 完整文档（6份）  

### 进行中
🔄 Phase 1.1: Event Store 实现

### 待开始
⏳ Phase 1.2: Patch System  
⏳ Phase 1.3: FSRS 集成  
⏳ Phase 2: Skill + LLM  
⏳ Phase 3: Learning Workflow  
⏳ Phase 4: 前端集成  
⏳ Phase 5: 完整学习系统  

---

## 🔗 关键资源

### 技术文档
- [Event Sourcing 权威指南](https://martinfowler.com/eaaDev/EventSourcing.html)
- [rust-genai 文档](https://docs.rs/genai)
- [fsrs 算法论文](https://github.com/open-spaced-repetition/fsrs-rs)
- [sqlx 异步教程](https://github.com/launchbadge/sqlx)

### 词典资源
- ECDICT: https://github.com/skywind3000/ECDICT
- Ceelog GPT4: https://github.com/Ceelog/DictionaryByGPT4
- Etymology DB: https://github.com/droher/etymology-db

---

## 💬 今日讨论要点

1. **为什么不直接用 Codex CLI 的 Agent 架构？**
   - Codex 是完整产品，Agent 系统与业务高度耦合
   - 无法直接抽取复用（Bazel 构建 + 内部依赖）
   - 我们的核心是"学习状态机"，不是通用 Agent

2. **为什么不用 Instructor + FastAPI（Python）？**
   - 引入 Python 依赖，增加复杂度
   - 桌面端应用应该是单一可执行文件
   - rust-genai 已足够强大（Structured Output 支持完善）

3. **为什么要 Event Sourcing？**
   - 支持版本回退（用户可以恢复到任意历史状态）
   - 支持模型升级后重新生成（Replay 所有事件）
   - 完整审计日志（调试/分析用户学习模式）
   - 支持 A/B 测试（对比不同 Prompt 效果）

---

**下次会话目标**: 完成 Event Store 基础实现，能够存储和读取事件流。
