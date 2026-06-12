# 前端大规模重构计划 - Phase 2

## 基于用户反馈的完整改进方案

### 核心问题总结

1. **设置页面混乱** - 2677行代码，无分类，中英混杂
2. **翻译引擎选择不明确** - 勾选了多个但不知道用的哪个
3. **OCR引擎选择不明确** - 同上
4. **页面功能混杂** - 浏览器扩展功能出现在桌面版
5. **缺少词典页面** - 需要独立的词典/生词本功能
6. **按钮堆在一起** - 需要侧边栏导航

---

## 🎯 重构目标

### 1. 设置页面完全重构 ⭐ 优先级最高

#### 当前问题
```
Settings.tsx (2677行)
├── 所有设置混在一起
├── 中英文混杂
├── 没有分类
├── 按钮全部堆在一起
└── 用户找不到想要的设置
```

#### 目标结构
```
Settings.tsx (使用侧边栏+Tab)
├── 左侧边栏导航
│   ├── 🌐 基础设置
│   ├── 🔄 翻译引擎
│   ├── 👁️ OCR设置
│   ├── ⚡ 快捷键
│   ├── 🎨 外观主题
│   └── 🔧 高级设置
└── 右侧内容区
    └── [根据选中项显示对应设置]
```

#### 翻译引擎设置改进 ⭐ 关键
**当前问题**：
- 勾选了Google、Youdao、DeepL，但不知道实际用的哪个
- 没有引擎优先级显示
- 没有当前使用引擎的指示

**解决方案**：
```tsx
翻译引擎设置
├── 路由策略选择 (必选)
│   ├── ⚪ 单引擎模式
│   │   └── 下拉选择：Google / Youdao / DeepL / ...
│   │       [只使用选中的引擎]
│   │
│   ├── 🔘 回退模式 (默认)
│   │   └── 引擎优先级列表 (可拖拽排序)
│   │       1. Google ✓ 已启用
│   │       2. Youdao ✓ 已启用
│   │       3. DeepL ✗ 未配置 (需要API Key)
│   │       [按顺序尝试，第一个成功就返回]
│   │
│   ├── ⚪ 并行模式
│   │   └── 选择引擎 (多选)
│   │       ☑ Google
│   │       ☑ Youdao
│   │       ☐ DeepL (未配置)
│   │       [同时调用，显示所有结果]
│   │
│   └── ⚪ 成本优先模式
│       └── 优先使用免费引擎，失败后尝试付费
│
└── 引擎详细配置
    ├── Google Translation (免费)
    │   ☑ 启用
    │   [无需配置]
    │
    ├── 有道翻译 (免费/付费)
    │   ☑ 启用
    │   ☐ 使用AI增强
    │
    ├── 彩云小译 (免费额度)
    │   ☑ 启用
    │   API Token: [••••••••••] [显示] [测试连接]
    │   剩余额度: 980,245 / 1,000,000 字符
    │
    ├── DeepL (付费)
    │   ☐ 启用 (需要API Key)
    │   API Key: [____________] [保存]
    │   ☐ Pro账户
    │   [获取API Key →]
    │
    └── ...其他引擎
```

#### OCR引擎设置改进 ⭐ 关键
**当前问题**：同翻译引擎，不知道用的哪个

**解决方案**：
```tsx
OCR引擎设置
├── 引擎选择 (单选)
│   🔘 Windows 原生 OCR (推荐)
│       ✅ 快速、准确、免费
│       
│   ⚪ 自动选择
│       会并行尝试多个引擎，可能较慢
│       
│   ⚪ 有道 OCR
│       ⚠️ 当前不可用 (API返回404)
│       
│   ⚪ Tesseract.js
│       离线引擎，速度较慢
│
├── 当前使用: Windows 原生 OCR
│
└── OCR参数
    刷新间隔: [2000] ms
    超时时间: [30] 秒
```

---

### 2. 创建词典页面 ⭐ 新功能

```
词典/生词本页面
├── 顶部搜索栏
│   └── [搜索词汇...]
│
├── 左侧：生词列表
│   ├── 过滤器
│   │   ├── 全部 (125)
│   │   ├── 今日新增 (8)
│   │   ├── 待复习 (23)
│   │   └── 已掌握 (94)
│   │
│   └── 生词卡片列表
│       ├── [abandon] ⭐
│       ├── [architecture] ⭐⭐
│       └── [brilliant] ⭐⭐⭐
│
└── 右侧：词汇详情
    ├── 单词: brilliant
    ├── 音标: /ˈbrɪliənt/
    ├── 词性: adj.
    ├── 释义
    │   ├── 💎 (形容词) 极好的，出色的
    │   └── 🌟 (形容词) 明亮的，闪耀的
    │
    ├── 例句
    │   └── It was a brilliant performance.
    │
    ├── 学习记录
    │   ├── 首次见于: 2026-06-10
    │   ├── 复习次数: 3次
    │   ├── 记忆强度: ⭐⭐⭐⭐☆ (80%)
    │   └── 下次复习: 2026-06-15
    │
    └── 操作按钮
        [标记为已掌握] [删除] [导出]
```

---

### 3. 主导航重构

#### 当前导航
```
顶部Tabs:
主页 | 历史 | 批量翻译 | 项目管理 | Hook监控 | 设置
```

#### 改进后的导航 (添加图标，更清晰)
```
顶部Tabs + 图标:
🏠 翻译     | 📚 词典     | 📜 历史     | 
📦 批量翻译  | 📂 项目    | 🔌 Hook监控 | 
⚙️ 设置
```

---

## 📁 文件结构规划

### 新增文件
```
src/pages/
├── Dictionary.tsx          (新建 - 词典页面)
└── Settings/               (新建目录 - 设置模块化)
    ├── SettingsLayout.tsx  (布局 + 侧边栏)
    ├── BasicSettings.tsx   (基础设置)
    ├── EngineSettings.tsx  (翻译引擎)
    ├── OcrSettings.tsx     (OCR设置)
    ├── HotkeySettings.tsx  (快捷键)
    ├── AppearanceSettings.tsx (外观)
    └── AdvancedSettings.tsx   (高级)

src/components/
├── Sidebar.tsx             (新建 - 侧边栏导航)
├── EngineCard.tsx          (新建 - 引擎配置卡片)
├── RoutingStrategySelector.tsx (新建 - 路由策略选择)
└── VocabularyCard.tsx      (新建 - 生词卡片)
```

### 重构文件
```
src/pages/
├── Settings.tsx            (重构 - 2677行 → 200行)
├── MainTranslator.tsx      (优化 - 按钮分组)
└── HookMonitor.tsx         (优化 - 布局改进)
```

---

## 🎨 设计规范

### 侧边栏导航设计
```tsx
<div className="flex h-full">
  {/* 左侧边栏 */}
  <div className="w-64 bg-bg-secondary border-r border-border">
    <nav className="p-4 space-y-1">
      <SidebarItem 
        icon={<Globe />} 
        label="基础设置" 
        active={true} 
      />
      <SidebarItem 
        icon={<Languages />} 
        label="翻译引擎" 
      />
      {/* ... */}
    </nav>
  </div>
  
  {/* 右侧内容区 */}
  <div className="flex-1 overflow-y-auto p-8">
    <Card title="基础设置">
      {/* 设置内容 */}
    </Card>
  </div>
</div>
```

### 引擎配置卡片设计
```tsx
<Card>
  <div className="flex items-center justify-between mb-4">
    <div className="flex items-center gap-3">
      <img src="/icons/google.svg" className="w-8 h-8" />
      <div>
        <h4>Google Translation</h4>
        <p className="text-xs text-text-secondary">
          免费 • 无需配置
        </p>
      </div>
    </div>
    
    <Switch checked={true} />
  </div>
  
  <div className="flex items-center gap-2">
    <Badge variant="success">已连接</Badge>
    <Badge variant="info">免费</Badge>
  </div>
</Card>
```

---

## 🔄 实施步骤

### Phase 1: 基础组件 (1小时) ✅ 已完成
- [x] Tabs组件
- [x] Card组件
- [ ] Sidebar组件
- [ ] Badge组件
- [ ] Switch组件

### Phase 2: 设置页面重构 (3小时)
**Step 1**: 创建Settings模块目录结构
```bash
mkdir -p src/pages/Settings
```

**Step 2**: 拆分Settings.tsx
```
Settings.tsx (2677行)
↓ 拆分为
├── SettingsLayout.tsx (100行 - 布局)
├── BasicSettings.tsx (200行)
├── EngineSettings.tsx (400行 - 最重要)
├── OcrSettings.tsx (150行)
├── HotkeySettings.tsx (200行)
├── AppearanceSettings.tsx (150行)
└── AdvancedSettings.tsx (300行)
```

**Step 3**: 实现引擎路由策略UI
- 单选按钮组：单引擎/回退/并行/成本优先
- 每种模式的详细说明和配置UI
- 当前使用引擎的明确指示

**Step 4**: 实现引擎配置卡片
- 每个引擎一个Card
- 显示状态：已启用/未配置/不可用
- API Key输入和测试连接按钮

### Phase 3: 词典页面 (2小时)
**Step 1**: 创建Dictionary.tsx基础结构
**Step 2**: 实现生词列表UI
**Step 3**: 实现词汇详情面板
**Step 4**: 集成现有生词本数据

### Phase 4: 其他页面优化 (1小时)
**Step 1**: MainTranslator按钮分组
**Step 2**: HookMonitor布局改进
**Step 3**: 统一所有页面间距和风格

### Phase 5: 测试和调优 (1小时)
**Step 1**: 功能测试
**Step 2**: 视觉一致性检查
**Step 3**: 响应式测试

---

## 📊 工作量估算

| 任务 | 预计时间 | 难度 |
|-----|---------|------|
| Phase 1: 基础组件 | 1小时 | 低 |
| Phase 2: 设置重构 | 3小时 | 高 |
| Phase 3: 词典页面 | 2小时 | 中 |
| Phase 4: 其他优化 | 1小时 | 低 |
| Phase 5: 测试调优 | 1小时 | 中 |
| **总计** | **8小时** | - |

---

## ✅ 验收标准

### 设置页面
- [ ] 有清晰的侧边栏导航
- [ ] 翻译引擎路由策略清晰可见
- [ ] 用户明确知道当前使用的引擎
- [ ] OCR引擎选择清晰明了
- [ ] 所有设置分类合理
- [ ] 中英文统一
- [ ] 无功能丢失

### 词典页面
- [ ] 生词列表可浏览
- [ ] 搜索功能正常
- [ ] 词汇详情显示完整
- [ ] 学习记录可查看

### 整体UI
- [ ] 所有页面间距统一（8px网格）
- [ ] 所有按钮有hover效果
- [ ] Dark/Light主题都正常
- [ ] 响应式布局正常

---

## 🚀 开始时间

**现在开始！**

预计完成时间：2026-06-13 早上6点

---

**制定者**: Claude Opus 4.8 (1M context)
**制定时间**: 2026-06-12 深夜
**基于**: 用户反馈 + PRODUCT_IMPROVEMENT_ANALYSIS.md + project-triage.md
