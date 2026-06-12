# 前端现代化改进计划

## 设计参考
- [GPTranslate](https://github.com/philberndt/GPTranslate) - 现代翻译应用UI
- [shadcn/ui](https://ui.shadcn.com/docs/components/card) - 组件设计系统
- [Tauri UI](https://github.com/agmmnn/tauri-ui-boilerplate) - 桌面应用模板

## 核心设计原则

### 1. 间距系统 (8px基准网格)
- 小间距：8px (p-2, gap-2)
- 中间距：16px (p-4, gap-4)
- 大间距：24px (p-6, gap-6)
- 超大间距：32px (p-8, gap-8)

### 2. 圆角系统
- 小圆角：4px (rounded)
- 中圆角：8px (rounded-lg)
- 大圆角：12px (rounded-xl)
- 全圆角：9999px (rounded-full)

### 3. 阴影系统
- 轻微：shadow-sm
- 标准：shadow-md
- 强烈：shadow-lg
- 超强：shadow-xl

### 4. 颜色系统（已有Tailwind主题）
- 主色：primary（蓝色）
- 背景：bg-primary, bg-secondary, bg-tertiary
- 文本：text-primary, text-secondary
- 边框：border

## 待改进页面

### 优先级 P0（严重影响用户体验）

#### 1. ✅ OCR区域（已完成）
- [x] 工具栏固定定位
- [x] 按钮尺寸增大
- [x] 默认关闭自动刷新

#### 2. 设置页面 Settings.tsx
**问题**：
- 设置项混杂，无分类
- 按钮没有图标
- 间距不统一
- 配色不一致

**改进方案**：
```
1. 使用标签式布局（Tabs）分类：
   - 基础设置（语言、快捷键）
   - 翻译引擎
   - OCR设置
   - 高级设置（代理、API端口）

2. 卡片化设计：
   - 每个设置组用Card包裹
   - 标题 + 描述 + 控件
   - 8px间距

3. 按钮现代化：
   - 图标 + 文字
   - hover效果
   - 加载状态

4. 输入框改进：
   - 更大的padding
   - 清晰的focus状态
   - 错误提示
```

#### 3. 主翻译界面 MainTranslator.tsx
**问题**：
- 按钮太小太多
- 布局拥挤
- 没有卡片分组

**改进方案**：
```
1. 双栏卡片布局：
   ┌─────────────┬─────────────┐
   │  输入卡片    │  输出卡片    │
   │  [文本框]   │  [结果]     │
   │  [工具栏]   │  [多引擎]   │
   └─────────────┴─────────────┘

2. 工具栏分组：
   - 语言选择区
   - 操作按钮区（翻译、清除）
   - 高级功能区（OCR、词典、语音）

3. 按钮改进：
   - 主要操作：大按钮 + 图标
   - 次要操作：图标按钮
   - Tooltip提示
```

### 优先级 P1（影响体验）

#### 4. Hook Monitor页面
**改进**：
- 结果卡片化
- 按钮分组
- 搜索框现代化

#### 5. 项目管理页面
**改进**：
- 列表项卡片化
- 操作按钮浮动
- 进度条美化

## 实施步骤

### Phase 1: 设置页面（2小时）
1. 创建Tabs组件
2. 重构设置分类
3. 卡片化每个设置组
4. 按钮现代化

### Phase 2: 主翻译界面（3小时）
1. 双栏卡片布局
2. 工具栏分组
3. 按钮现代化
4. 添加动画过渡

### Phase 3: 其他页面（2小时）
1. Hook Monitor卡片化
2. 项目管理优化
3. 统一间距和颜色

## 现代化设计要点

### 按钮设计
```tsx
// 主要按钮
<button className="px-4 py-2.5 bg-primary text-white rounded-lg hover:bg-primary/90 
                   transition-colors flex items-center gap-2 shadow-sm">
  <Icon size={18} />
  <span>操作</span>
</button>

// 次要按钮
<button className="px-3 py-2 bg-bg-secondary text-text-primary rounded-lg 
                   hover:bg-bg-tertiary transition-colors border border-border">
  次要操作
</button>

// 图标按钮
<button className="p-2 rounded-lg hover:bg-bg-secondary transition-colors">
  <Icon size={18} />
</button>
```

### 卡片设计
```tsx
<div className="bg-bg-secondary border border-border rounded-xl p-6 shadow-sm">
  <h3 className="text-lg font-semibold text-text-primary mb-4">标题</h3>
  <p className="text-sm text-text-secondary mb-4">描述</p>
  {/* 内容 */}
</div>
```

### 输入框设计
```tsx
<input 
  className="w-full px-4 py-3 bg-bg-tertiary text-text-primary border border-border 
             rounded-lg focus:border-primary focus:ring-2 focus:ring-primary/20 
             outline-none transition-all"
  placeholder="输入内容..."
/>
```

## 配色方案（使用现有Tailwind变量）

### Light Mode
- 主色：#3B82F6 (blue-500)
- 背景：#FFFFFF, #F9FAFB, #F3F4F6
- 文字：#111827, #6B7280
- 边框：#E5E7EB

### Dark Mode
- 主色：#60A5FA (blue-400)
- 背景：#1F2937, #111827, #0F172A
- 文字：#F9FAFB, #9CA3AF
- 边框：#374151

## 动画效果

### Hover效果
```css
transition-colors duration-200
hover:bg-opacity-90
```

### 加载动画
```css
animate-spin (for spinner)
animate-pulse (for skeleton)
```

### 过渡效果
```css
transition-all duration-300
```

## 参考资源

### 组件库
- [shadcn/ui Card](https://ui.shadcn.com/docs/components/card)
- [shadcn/ui Button](https://ui.shadcn.com/docs/components/button-group)
- [shadcn/ui Tabs](https://ui.shadcn.com/docs/components/tabs)

### 设计系统
- [Design Systems Spacing](https://www.designsystems.com/space-grids-and-layouts/)
- [8pt Grid System](https://spec.fm/specifics/8-pt-grid)

### 实际应用
- [GPTranslate UI](https://github.com/philberndt/GPTranslate)
- [Tauri UI Examples](https://github.com/agmmnn/tauri-ui-boilerplate)

## 测试检查清单

- [ ] 所有按钮都有hover效果
- [ ] 所有输入框都有focus状态
- [ ] 间距统一使用8px网格
- [ ] 卡片都有阴影和圆角
- [ ] 颜色符合主题系统
- [ ] 动画流畅不卡顿
- [ ] 响应式布局正常
- [ ] Dark/Light模式都正常

---

**制定日期**: 2026-06-12
**预计完成**: 2026-06-13
**负责人**: Claude Opus 4.8
