# 词汇学习系统 - 快速启动指南

## 🚀 启动步骤

### 1. 启动开发服务器
```bash
cd E:/Code/ai/moontranslator
pnpm tauri dev
```

### 2. 初始化数据（首次使用）
在另一个终端运行：
```bash
cd E:/Code/ai/moontranslator/src-tauri
cargo run --example data_init_demo
```

这将导入：
- 15,000 核心高频词
- 70,000+ 词根数据

### 3. 访问应用

应用会自动打开 Tauri 窗口。

如果没有自动打开，访问: http://localhost:5173/

### 4. 使用词汇学习功能

1. 点击左侧边栏的 📖 **Vocabulary** 图标
2. 选择 🎓 **AI Learning** 标签
3. 开始使用：
   - **Browse Vocabulary**: 浏览核心词库
   - **Review**: 复习待学习卡牌
   - **Statistics**: 查看学习统计

## 🐛 故障排查

### 如果出现模块导入错误

1. 停止开发服务器 (Ctrl+C)
2. 清理缓存:
```bash
rm -rf node_modules/.vite
rm -rf src-tauri/target/debug
```
3. 重新启动:
```bash
pnpm tauri dev
```

### 如果页面空白

1. 打开浏览器开发者工具 (F12)
2. 查看 Console 标签的错误信息
3. 刷新页面 (F5)

### 如果 Tauri 窗口没有打开

直接访问: http://localhost:5173/

前端也可以在浏览器中使用（部分功能需要 Tauri 环境）。

## ✅ 验证安装

### 检查前端是否启动
访问 http://localhost:5173/ 应该能看到应用界面。

### 检查后端是否编译
在终端应该看到：
```
Finished `dev` profile [unoptimized + debuginfo] target
```

### 检查词汇学习页面
1. 进入 Vocabulary 页面
2. 点击 AI Learning 标签
3. 应该能看到三个按钮:
   - Browse Vocabulary
   - Review (0)
   - Statistics

## 📝 注意事项

1. **首次使用必须运行数据导入**，否则词库为空
2. **需要配置 LLM API** 才能使用 AI 生成功能
3. 所有数据存储在本地 SQLite 数据库中

## 🎯 下一步

配置 LLM (可选):
1. 进入 Settings 页面
2. 找到 LLM 配置
3. 填入 OpenAI 兼容的 API Key 和 Base URL

然后就可以使用完整的 AI 功能了！
