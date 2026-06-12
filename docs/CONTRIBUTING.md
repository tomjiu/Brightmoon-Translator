# Moon Translator 贡献指南

感谢你对 Moon Translator 项目的关注!本文档将帮助你了解如何参与项目开发。

---

## 开发环境设置

### 前置要求

- **Rust**: stable 版本 (1.70+)
- **Node.js**: 18+
- **npm**: 9+ (推荐)
- **Windows 10/11**: 项目依赖 Windows UI Automation API

### 安装步骤

1. **克隆仓库**

```bash
git clone https://github.com/your-username/moontranslator.git
cd moontranslator
```

2. **安装前端依赖**

```bash
npm install
```

3. **启动开发服务器**

```bash
npm tauri dev
```

这会同时启动 Vite 前端开发服务器和 Tauri 后端。

### 开发工具推荐

- **VS Code** + 扩展:
  - rust-analyzer (Rust 语言支持)
  - ESLint (JavaScript/TypeScript 检查)
  - Tailwind CSS IntelliSense
  - Tauri Extension

---

## 项目结构

```
moontranslator/
├── src/                    # React 前端代码
├── src-tauri/              # Rust 后端代码
├── extension/              # 浏览器扩展
├── docs/                   # 文档
├── scripts/                # 工具脚本
└── public/                 # 静态资源
```

---

## 代码规范

### TypeScript / React

1. **使用 TypeScript**: 所有新代码必须使用 TypeScript
2. **类型定义**: 禁止使用 `any`，必须定义明确的类型
3. **组件规范**:
   - 使用函数组件 + Hooks
   - 组件文件使用 PascalCase 命名
   - 一个文件一个组件

4. **代码风格**:
   ```typescript
   // 正确
   interface TranslateRequest {
     text: string;
     from: string;
     to: string;
   }

   const translate = async (request: TranslateRequest): Promise<TranslateResponse> => {
     // ...
   };

   // 错误
   const translate = async (request: any) => {
     // ...
   };
   ```

5. **状态管理**: 使用 Zustand，按功能拆分 store
6. **样式**: 使用 Tailwind CSS，避免自定义 CSS

### Rust

1. **代码风格**: 遵循 `rustfmt` 默认配置
2. **错误处理**:
   - 使用 `anyhow::Result` 处理错误
   - 使用 `thiserror` 定义错误类型
   - 避免 `unwrap()`，使用 `?` 传播错误

3. **异步编程**:
   - 使用 `tokio` 异步运行时
   - 使用 `async-trait` 定义异步 trait
   - 避免阻塞操作

4. **代码示例**:
   ```rust
   use anyhow::Result;
   use async_trait::async_trait;

   #[async_trait]
   pub trait TranslationEngine: Send + Sync {
       async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String>;
       fn name(&self) -> &str;
   }
   ```

5. **命名规范**:
   - 模块: snake_case
   - 类型: PascalCase
   - 函数/变量: snake_case
   - 常量: SCREAMING_SNAKE_CASE

### 浏览器扩展

1. **JavaScript**: 使用 ES6+ 语法
2. **兼容性**: 支持 Chrome MV3 和 Firefox
3. **权限最小化**: 仅申请必要权限

---

## 提交规范

### Commit Message 格式

使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范:

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Type 类型

- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式调整 (不影响功能)
- `refactor`: 重构 (不是新功能也不是修复)
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建/工具链更新
- `ci`: CI 配置更新

### 示例

```
feat(ocr): 添加有道 OCR 支持

- 集成有道 OCR API
- 支持中英文混合识别
- 添加 OCR 结果缓存

Closes #123
```

```
fix(translation): 修复 UTF-8 字符串切片 panic

修复在处理包含多字节字符的文本时发生的 panic。
使用 char_indices() 替代字节索引。

Fixes #456
```

---

## Pull Request 流程

### 1. 创建分支

```bash
# 从 master 创建功能分支
git checkout -b feature/your-feature master

# 或修复分支
git checkout -b fix/your-fix master
```

分支命名:
- `feature/xxx`: 新功能
- `fix/xxx`: 修复
- `docs/xxx`: 文档
- `refactor/xxx`: 重构

### 2. 开发和测试

```bash
# 前端检查
npm check
npm lint
npm test

# 后端检查
cd src-tauri
cargo check
cargo clippy -- -D warnings
cargo test
```

### 3. 提交代码

```bash
git add .
git commit -m "feat(xxx): your feature description"
```

### 4. 推送和创建 PR

```bash
git push origin feature/your-feature
```

然后在 GitHub 上创建 Pull Request。

### 5. PR 要求

- **标题**: 遵循 Commit Message 规范
- **描述**: 说明改动内容和原因
- **测试**: 说明如何测试
- **截图**: 如果有 UI 改动，提供截图
- **关联 Issue**: 使用 `Closes #xxx` 或 `Fixes #xxx`

### 6. 代码审查

- 至少需要一个维护者审核
- CI 检查必须通过
- 解决所有 review comments

---

## Issue 模板

### Bug Report

```markdown
## Bug 描述

简要描述 bug

## 复现步骤

1. 打开应用
2. 点击 '...'
3. 输入 '...'
4. 看到错误

## 期望行为

描述期望的行为

## 实际行为

描述实际的行为

## 环境信息

- OS: Windows 11
- App Version: 0.1.0
- Rust Version: 1.70.0
- Node Version: 18.0.0

## 日志

```
粘贴相关日志
```

## 截图

如果适用，添加截图
```

### Feature Request

```markdown
## 功能描述

简要描述功能

## 使用场景

描述使用场景

## 建议实现

如果有想法，描述建议的实现方式

## 替代方案

描述考虑过的替代方案

## 附加信息

其他相关信息
```

---

## 开发指南

### 添加新翻译引擎

1. 在 `src-tauri/src/engine/` 创建新文件:

```rust
// src-tauri/src/engine/myengine.rs
use super::TranslationEngine;
use anyhow::Result;
use async_trait::async_trait;

pub struct MyEngine {
    client: reqwest::Client,
}

impl MyEngine {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TranslationEngine for MyEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        // 实现翻译逻辑
        Ok("translated".to_string())
    }

    fn name(&self) -> &str {
        "MyEngine"
    }
}
```

2. 在 `src-tauri/src/engine/mod.rs` 注册:

```rust
pub mod myengine;

// 在 Router::new() 中添加
if config.engines.myengine.enabled {
    engines.push(Arc::new(myengine::MyEngine::new()));
}
```

3. 在 `src-tauri/src/models/config.rs` 添加配置:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyEngineConfig {
    pub enabled: bool,
    pub api_key: String,
}
```

### 添加新的 Tauri 命令

1. 在 `src-tauri/src/commands/` 创建或编辑文件:

```rust
// src-tauri/src/commands/my_cmd.rs
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn my_command(
    state: State<'_, AppState>,
    param: String,
) -> Result<String, String> {
    // 实现逻辑
    Ok("result".to_string())
}
```

2. 在 `src-tauri/src/lib.rs` 注册:

```rust
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    commands::my_cmd::my_command,
])
```

3. 在前端调用:

```typescript
import { invoke } from "@tauri-apps/api/core";

const result = await invoke("my_command", { param: "value" });
```

### 添加前端页面

1. 在 `src/pages/` 创建页面组件
2. 在 `src/App.tsx` 添加路由:

```tsx
type Page = "translator" | "settings" | "mypage";

// 在 MainApp 中添加
{page === "mypage" && <MyPage />}
```

3. 在导航栏添加入口:

```tsx
const navItems: NavItem[] = [
  // ... 其他项
  { id: "mypage", icon: MyIcon, label: t("nav.mypage"), group: "system" },
];
```

---

## 测试

### 前端测试

```bash
# 运行所有测试
npm test

# 监听模式
npm test:watch
```

测试文件命名: `*.test.ts` 或 `*.test.tsx`

```typescript
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import MyComponent from "./MyComponent";

describe("MyComponent", () => {
  it("renders correctly", () => {
    render(<MyComponent />);
    expect(screen.getByText("Hello")).toBeInTheDocument();
  });
});
```

### 后端测试

```bash
cd src-tauri

# 运行所有测试
cargo test

# 运行特定测试
cargo test test_name
```

测试文件放在 `src-tauri/tests/` 或使用 `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate() {
        // 测试逻辑
    }

    #[tokio::test]
    async fn test_async_translate() {
        // 异步测试
    }
}
```

---

## CI/CD

项目使用 GitHub Actions 进行 CI:

### CI 检查

- **Rust Check**: cargo check, clippy, test
- **Frontend Check**: TypeScript check, lint, build
- **Extension Check**: manifest 验证, JS 语法检查

### 触发条件

- Push 到 `master` 或 `develop` 分支
- Pull Request 到 `master` 或 `develop` 分支

---

## 发布流程

1. 更新版本号:
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`

2. 创建 Git tag:

```bash
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

3. GitHub Actions 自动构建和发布

---

## 获取帮助

- **Issues**: 提交 bug 或功能请求
- **Discussions**: 讨论问题和想法
- **Email**: 联系维护者

---

## 行为准则

- 尊重所有参与者
- 接受建设性批评
- 专注于对社区最有利的事情
- 对他人表示同理心

---

## 许可证

参与即表示你同意你的贡献将在项目的开源许可证下发布。
