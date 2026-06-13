# MoonTranslator 完整开发计划 - 2026-06-12

## 📋 用户提出的所有问题汇总

### 一、产品定位和功能问题

#### 1. 产品定位转型
**问题**: 当前是翻译工具，但希望做成智能语言学习平台
**核心需求**:
- 词典学习系统（导入牛津、有道等词典）
- AI辅助学习（词根、情景、记忆技巧）
- 个性化学习（每个人学习方式不同，卡片和策略应个性化）
- 遗忘曲线和间隔重复
- 打卡日程功能
- 云同步（Cloudflare R2 + Workers 或 GitHub/WebDAV）

#### 2. UI/UX问题
**问题列表**:
- 暗黑模式配色老土（小问题）
- 功能分类混乱（浏览器功能出现在桌面版）
- 设置页面混乱，中英文混杂
- TTS设置存在但功能不完整
- 语音识别未实现
- **窗口上边栏白边问题**（系统标题栏与主题不匹配）

#### 3. 功能缺失和改进
**问题列表**:
- Hook模式需要手动输入进程号（应该图形化选择）
- PDF等长文本翻译未测试
- 生词本功能简单，需要升级为智能词典学习系统
- 插件市场和插件管理重复，应该合并
- 翻译设置混乱
- 大文本翻译后应选择是否加入学习库
- 选中文字应出现浮动操作（有道风格）
- 剪贴板粘贴应能快速保存到单词本

#### 4. 浏览器-桌面联动
**需求**:
- 浏览器扩展读取页面内容
- 保存文章作为学习语料
- 快速保存单词到生词本
- 浏览器和桌面数据同步

#### 5. 跨平台需求
**目标平台**:
- 桌面版（Windows/Mac/Linux）- 当前
- 手机端（iOS/Android）- 必做
- 微信小程序 - 可能

#### 6. 技术架构问题
**明确要求**:
- 希望用Rust开发（性能强劲）
- 需要本地AI模型（轻量，用于翻译，不要重框架）
- 词典学习的AI应该调用API大模型（OpenAI/Claude/Gemini/DeepSeek）
- 参考成熟开源项目，不要重复造轮

---

### 二、实时发现的问题

#### 7. OCR截屏卡顿
**问题**: 点击OCR按钮后卡1-2秒才能截屏
**原因**: 同步截图+编码+写盘（总计1.2秒）
**期望**: 秒开

#### 8. OCR监听期间弹窗闪烁
**问题**: OCR区域帧在连续监听时闪烁
**原因**: 频繁无条件状态更新
**状态**: ✅ 已修复

#### 9. Youdao OCR返回404
**问题**: 有道OCR API返回404
**影响**: 无（WinRT OCR正常工作）
**是否修复**: 可选

#### 10. 引擎不足
**问题**: 缺少更多翻译引擎（如彩云）和更好的OCR
**需求**: 
- 彩云小译
- PaddleOCR（本地，准确率高）
- 百度OCR（云端备用）

#### 11. DeepL引擎疑问
**问题**: "我的DeepL和DeepLX去哪里了？"
**状态**: 需要确认是否已经实现

---

## 🔍 DeepL/DeepLX状态确认

### 检查代码中的实现

#### DeepL配置（已实现）✅
```rust
// src-tauri/src/models/config.rs
pub struct DeepLConfig {
    pub enabled: bool,
    pub api_key: String,
    pub pro: bool,
}
```

#### DeepLX配置（已实现）✅
```rust
pub struct DeepLXConfig {
    pub enabled: bool,
    pub api_key: Option<String>,
    pub pro: bool,
}
```

#### 引擎实现（需确认）
```
src-tauri/src/engine/
├── deepl.rs      ← 应该存在
├── deeplx.rs     ← 应该存在
└── mod.rs        ← 应该注册了这两个引擎
```

**结论**: DeepL和DeepLX的**配置已实现**，需要确认**引擎代码是否完整**

---

## 📊 优先级分类

### P0 - 核心价值（必须做）
1. 智能词典学习系统（个性化、AI辅助、间隔重复）
2. PaddleOCR本地集成
3. 浏览器-桌面深度联动

### P1 - 重要改进（强烈建议）
4. OCR截屏卡顿修复（启动预热）
5. 自定义标题栏（消除白边）
6. 进程选择器（图形化，不用输入PID）
7. 设置页面重组（中英文统一）
8. 彩云小译集成
9. 百度OCR集成
10. 大文本学习库保存选项
11. 选中文字浮动工具栏

### P2 - 体验优化（建议做）
12. 修复Youdao OCR（可选）
13. 插件系统简化
14. PDF翻译测试优化
15. TTS功能补全
16. 隐藏平台特定功能

### P3 - 可选优化
17. 暗色主题优化
18. 语音识别功能

---

## 🚀 完整开发计划（3个月）

### Week 1-2: Quick Wins（立即改善体验）

#### Week 1
**Day 1**（今天）:
- [ ] 确认DeepL/DeepLX引擎实现状态
- [ ] OCR启动预热（1天）
  - 应用启动后1秒预热截图
  - 智能缓存检查（30秒有效期）
  - 预期: 秒开

**Day 2-3**:
- [ ] 彩云小译集成（1.5天）
  - 实现CaiyunEngine
  - 添加配置UI
  - 测试长文本翻译

**Day 4-5**:
- [ ] 百度OCR集成（1.5天）
  - 实现BaiduOCR
  - Access token管理
  - 配置UI

#### Week 2
**Day 1-2**:
- [ ] 进程选择器（1.5天）
  - 读取系统进程列表
  - 图形化选择对话框
  - 搜索和筛选功能

**Day 3**:
- [ ] 设置页面统一（1天）
  - 中英文统一
  - 分类重组
  - 移除混杂内容

**Day 4-5**:
- [ ] 自定义标题栏（1.5天）
  - decorations: false
  - TitleBar组件
  - 跨平台测试

---

### Week 3-4: 核心引擎集成

#### Week 3: PaddleOCR集成 ⭐⭐⭐
**Day 1-2**:
- [ ] PaddleOCR ONNX模型下载和测试
  - 选择轻量模型（8.6MB）
  - 测试识别准确率

**Day 3-4**:
- [ ] Rust ONNX Runtime集成
  - 添加ort依赖
  - 模型加载和推理
  - 图像预处理

**Day 5**:
- [ ] 优化和测试
  - 性能优化（目标<200ms）
  - 多语言测试
  - 与WinRT OCR对比

#### Week 4: 引擎完善
**Day 1-2**:
- [ ] 确认并修复DeepL/DeepLX（如果有问题）
- [ ] 测试所有翻译引擎
- [ ] 引擎优先级配置

**Day 3-5**:
- [ ] 修复Youdao OCR（可选）
  - 官方API签名实现
  - 或逆向新端点

---

### Week 5-7: 词典学习系统（核心）

#### Week 5: MDX词典集成
**Day 1-2**:
- [ ] 词典格式调研
  - MDX格式解析（mdict-parser）
  - 词典资源收集（牛津、柯林斯等）

**Day 3-5**:
- [ ] 词典导入功能
  - 文件选择和解析
  - 索引构建（SQLite）
  - 词条查询界面

#### Week 6: AI学习辅助
**Day 1-2**:
- [ ] AI API适配层
  - 基于Vercel AI SDK
  - 支持OpenAI/Claude/Gemini/DeepSeek

**Day 3-5**:
- [ ] 用户画像系统
  - 学习风格识别（视觉/听觉/读写/动觉）
  - 学习历史追踪
  - 动态难度调整

#### Week 7: 间隔重复算法
**Day 1-3**:
- [ ] Anki SM-2算法实现（Rust）
  - 基础算法
  - 个性化调整
  - 复习计划生成

**Day 4-5**:
- [ ] 打卡日历功能
  - 连续学习统计
  - 学习目标设置
  - 激励机制

---

### Week 8-9: 浏览器联动

#### Week 8: 浏览器扩展增强
**Day 1-2**:
- [ ] 页面内容保存
  - 提取主要内容
  - 自动提取关键词汇
  - 发送到桌面应用

**Day 3-4**:
- [ ] 选中文字浮动工具栏
  - 监听文本选择
  - 显示浮动按钮（翻译/加词/朗读）
  - 快速保存到生词本

**Day 5**:
- [ ] 剪贴板智能识别
  - 识别单词/句子
  - 自动弹出保存提示

#### Week 9: 语料库系统
**Day 1-3**:
- [ ] 语料库数据结构
  - 文章存储
  - 词汇提取
  - 上下文关联

**Day 4-5**:
- [ ] 大文本处理
  - 翻译后保存选项
  - 词汇分析
  - 难度评估

---

### Week 10-11: 云同步

#### Week 10: Cloudflare R2 + Workers
**Day 1-2**:
- [ ] R2存储配置
  - Bucket创建
  - 认证配置

**Day 3-4**:
- [ ] Workers API开发
  - 上传/下载接口
  - 冲突解决（Last-Write-Wins）

**Day 5**:
- [ ] 客户端同步逻辑
  - 增量同步
  - 错误处理

#### Week 11: 同步完善
**Day 1-3**:
- [ ] WebDAV支持（已有基础，补全）
  - 坚果云等云盘支持
  - 配置UI

**Day 4-5**:
- [ ] 同步状态显示
  - 进度显示
  - 冲突提示
  - 手动同步按钮

---

### Week 12: 移动端准备

#### Tauri Mobile配置
**Day 1-3**:
- [ ] Tauri Mobile环境配置
  - iOS开发环境
  - Android开发环境
  - 构建测试

**Day 4-5**:
- [ ] UI适配开始
  - 响应式布局
  - 触摸手势
  - 移动端特定功能

---

## 🎯 立即行动（今天）

### 任务1: 确认DeepL/DeepLX状态（30分钟）
```bash
# 检查引擎文件是否存在
ls src-tauri/src/engine/deepl.rs
ls src-tauri/src/engine/deeplx.rs

# 检查是否注册
grep -rn "deepl\|deeplx" src-tauri/src/engine/mod.rs

# 测试是否可用
# 在配置中启用DeepL/DeepLX，测试翻译
```

### 任务2: OCR启动预热（4小时）
```rust
// src-tauri/src/lib.rs
.setup(|app| {
    let handle = app.handle().clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let _ = commands::capture::prepare_screenshot_snapshot().await;
        tracing::info!("Screenshot cache warmed up");
    });
    Ok(())
})
```

### 任务3: 彩云小译集成（4小时）
```rust
// src-tauri/src/engine/caiyun.rs
pub struct CaiyunEngine {
    api_token: String,
}

impl TranslationEngine for CaiyunEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        // 实现API调用
    }
}
```

---

## 📈 预期成果（3个月后）

### 核心功能
✅ 智能词典学习系统（个性化、AI辅助）  
✅ 完整的翻译引擎矩阵（7+引擎）  
✅ 顶级OCR系统（PaddleOCR + 云端备用）  
✅ 浏览器-桌面深度联动  
✅ 云同步（多方案）  
✅ 间隔重复学习算法  
✅ 打卡日历系统  

### 技术栈
✅ Rust核心（性能强劲）  
✅ 本地AI模型（PaddleOCR）  
✅ API大模型集成（学习辅助）  
✅ 基于成熟开源项目（Anki/Candle等）  

### 用户体验
✅ 秒开OCR  
✅ 无白边界面  
✅ 流畅不卡顿  
✅ 个性化学习  
✅ 跨平台同步  

---

## 💰 成本评估

### 开发时间
- **Quick Wins**: 2周
- **核心功能**: 7周
- **云同步**: 2周
- **移动端准备**: 1周
- **总计**: 12周（3个月）

### API成本（月）
- 彩云小译: 免费100万字
- 百度OCR: 免费1000次/天
- AI大模型: 按用户使用量（建议用户自行配置key）
- Cloudflare R2: 免费10GB
- **总计**: 基本免费

---

## 🎓 参考项目清单

### 已确认可用
1. **Anki** - `github.com/ankitects/anki`
   - 学习算法: `rslib/src/scheduler/`
   - 数据库设计
   - 云同步协议

2. **Candle** - `github.com/huggingface/candle`
   - Rust ML框架
   - 本地模型推理

3. **PaddleOCR** - `github.com/PaddlePaddle/PaddleOCR`
   - ONNX模型
   - 识别算法

4. **GoldenDict** - `github.com/goldendict/goldendict`
   - MDX解析: `src/mdx.cc`
   - 词典索引

5. **Vercel AI SDK** - `github.com/vercel/ai`
   - 多提供商统一接口

6. **PyGlossary** - `github.com/ilius/pyglossary`
   - 词典格式转换

---

## ✅ 行动检查清单

### 今天（Day 1）
- [ ] 确认DeepL/DeepLX状态
- [ ] OCR启动预热实现
- [ ] 彩云小译集成开始
- [ ] 推送所有文档到GitHub（30个提交）

### 本周（Week 1）
- [ ] 完成OCR预热
- [ ] 完成彩云小译
- [ ] 完成百度OCR
- [ ] 测试所有新引擎

### 本月（Month 1）
- [ ] 完成Quick Wins
- [ ] 完成PaddleOCR集成
- [ ] 开始词典学习系统

---

## 🎯 成功指标

### 技术指标
- OCR响应时间: <50ms（目标秒开）
- 翻译引擎: 7+可用
- OCR准确率: >95%（PaddleOCR）
- 应用启动: <3秒

### 用户指标
- 学习留存率: >60%（目标）
- 每日活跃: >30分钟
- 单词记忆: 符合遗忘曲线预期

---

**计划状态**: ✅ 就绪  
**开始日期**: 2026-06-12  
**预计完成**: 2026-09-12（3个月）  
**当前阶段**: Week 1 Day 1  

**需要立即开始吗？** 🚀

---

**制定者**: Claude Opus 4.8 (1M context)  
**文档版本**: v1.0  
**最后更新**: 2026-06-12
