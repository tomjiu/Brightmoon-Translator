# Moon Translator 故障排除指南

本文档帮助你解决使用 Moon Translator 时遇到的常见问题。

---

## 目录

- [常见问题](#常见问题)
  - [安装和启动](#安装和启动)
  - [翻译功能](#翻译功能)
  - [OCR 功能](#ocr-功能)
  - [悬浮窗](#悬浮窗)
  - [浏览器扩展](#浏览器扩展)
  - [快捷键](#快捷键)
  - [性能问题](#性能问题)
- [错误代码](#错误代码)
- [调试技巧](#调试技巧)
- [日志分析](#日志分析)
- [获取帮助](#获取帮助)

---

## 常见问题

### 安装和启动

#### Q: 应用无法启动，没有任何反应

**可能原因**:
1. 缺少 Visual C++ 运行时
2. Windows 版本过低
3. 防火墙/杀毒软件阻止

**解决方案**:

1. 安装 [Visual C++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)

2. 确认 Windows 版本:
```powershell
winver
```
需要 Windows 10 1809 或更高版本。

3. 将应用添加到杀毒软件白名单。

---

#### Q: 启动时提示 "Failed to initialize WebView"

**可能原因**:
1. WebView2 运行时未安装
2. WebView2 版本过旧

**解决方案**:

安装或更新 [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

---

#### Q: 开发模式启动失败

**错误信息**: `error: linking with link.exe failed`

**解决方案**:

1. 安装 Visual Studio Build Tools
2. 确认 Rust 工具链:
```bash
rustup show
```
确保使用 `stable-x86_64-pc-windows-msvc`

3. 清理并重建:
```bash
cd src-tauri
cargo clean
cargo build
```

---

### 翻译功能

#### Q: 翻译失败，提示 "No translation engine available"

**可能原因**:
1. 没有启用任何翻译引擎
2. LLM API Key 未配置

**解决方案**:

1. 打开设置页面，检查引擎启用状态
2. 配置至少一个引擎:
   - Google: 无需配置，直接启用
   - LLM: 需要配置 API Key
   - 有道: 默认启用，无需配置

3. 检查配置文件:
```
%APPDATA%\moontranslator\config.json
```

---

#### Q: LLM 翻译返回空结果

**可能原因**:
1. API Key 无效或过期
2. API 端点不可达
3. 模型名称错误

**解决方案**:

1. 验证 API Key:
```bash
curl https://api.deepseek.com/v1/models \
  -H "Authorization: Bearer sk-xxx"
```

2. 检查网络连接:
```bash
ping api.deepseek.com
```

3. 确认模型名称:
   - DeepSeek: `deepseek-chat`
   - OpenAI: `gpt-3.5-turbo` 或 `gpt-4`

---

#### Q: 翻译速度很慢

**可能原因**:
1. 网络延迟高
2. 使用了慢速引擎
3. 请求超时设置过长

**解决方案**:

1. 使用代理:
   设置 → 代理 → 启用代理

2. 切换路由策略:
   - `latency_first`: 延迟优先
   - `primary_only`: 仅主引擎

3. 减少请求超时:
   设置 → 高级 → 请求超时 (默认 30 秒)

---

#### Q: 翻译结果不准确

**解决方案**:

1. 尝试不同引擎:
   - LLM (DeepSeek/GPT): 适合长文本、上下文理解
   - DeepL: 欧洲语言质量高
   - Google: 覆盖语言多

2. 使用术语表:
   设置 → 术语表 → 添加专业术语

3. 自定义 Prompt:
   设置 → LLM → 自定义 Prompt

4. 使用并行对比模式:
   设置 → 路由策略 → Parallel Compare

---

### OCR 功能

#### Q: OCR 截图快捷键无反应

**可能原因**:
1. 快捷键被其他应用占用
2. 全局快捷键注册失败

**解决方案**:

1. 检查快捷键设置:
   设置 → 快捷键 → OCR 翻译

2. 更换快捷键:
   尝试使用其他组合键

3. 以管理员权限运行 (某些应用需要)

---

#### Q: OCR 识别结果为空

**可能原因**:
1. 截图区域不包含文字
2. 图片质量太差
3. 语言不支持

**解决方案**:

1. 确保截图区域包含清晰的文字
2. 调整截图区域大小
3. 对于中文，使用系统 OCR (Windows 原生)
4. 尝试有道 OCR (需要网络)

---

#### Q: OCR 识别不准确

**解决方案**:

1. 确保截图区域只包含文字
2. 避免截取背景复杂的区域
3. 增大截图区域
4. 使用高分辨率屏幕

---

#### Q: 有道 OCR 报错

**错误信息**: `OCR request failed: 403`

**可能原因**:
1. OCR API Key 无效
2. 请求频率超限

**解决方案**:

1. 检查有道 OCR 配置:
   设置 → 引擎 → 有道 → OCR 配置

2. 使用默认 Key (内置):
   默认 Key 有频率限制，等待一段时间后重试

3. 配置自己的有道 OCR Key

---

### 悬浮窗

#### Q: 悬浮窗不显示

**可能原因**:
1. 悬浮窗被其他窗口遮挡
2. 悬浮窗在屏幕外

**解决方案**:

1. 按 `Ctrl+Shift+Escape` 切换穿透模式
2. 重启应用
3. 检查多显示器设置

---

#### Q: 悬浮窗无法选中文字

**可能原因**:
1. 鼠标穿透模式开启
2. 悬浮窗层级问题

**解决方案**:

1. 按 `Ctrl+Shift+Escape` 关闭穿透模式
2. 点击悬浮窗内容区域
3. 调整悬浮窗层级:
   设置 → 悬浮窗 → 层级

---

#### Q: 悬浮窗位置不正确

**解决方案**:

1. 手动拖拽调整位置
2. 重置悬浮窗位置:
   设置 → 悬浮窗 → 重置位置
3. 检查跟随模式设置

---

### 浏览器扩展

#### Q: 扩展无法连接到桌面应用

**可能原因**:
1. API 服务器未启动
2. 端口被占用
3. 防火墙阻止

**解决方案**:

1. 启用 API 服务器:
   设置 → API 服务器 → 启用

2. 检查端口:
```powershell
netstat -ano | findstr :60828
```

3. 添加防火墙例外

---

#### Q: 划词翻译不工作

**可能原因**:
1. 权限不足
2. Content Script 未加载

**解决方案**:

1. 检查扩展权限:
   chrome://extensions → Moon Translator → 权限

2. 刷新页面

3. 检查控制台错误:
   F12 → Console

---

#### Q: 整页翻译很慢

**可能原因**:
1. 页面内容太多
2. 网络延迟
3. 引擎限制

**解决方案**:

1. 减少翻译段落数量
2. 使用更快的引擎
3. 配置 API Key 提高速度

---

### 快捷键

#### Q: 快捷键不生效

**可能原因**:
1. 快捷键被其他应用占用
2. 应用未获得焦点
3. 全局快捷键注册失败

**解决方案**:

1. 更换快捷键:
   设置 → 快捷键

2. 以管理员权限运行

3. 检查系统快捷键设置

---

#### Q: 如何自定义快捷键

**操作步骤**:

1. 打开设置页面
2. 点击 "快捷键" 选项卡
3. 点击要修改的快捷键
4. 按下新的快捷键组合
5. 保存

**支持的修饰键**: Ctrl, Shift, Alt, Super (Win)

**支持的按键**: A-Z, 0-9, F1-F12, Space, Enter, Tab, Escape 等

---

### 性能问题

#### Q: 应用占用内存很高

**可能原因**:
1. 缓存过大
2. 历史记录过多
3. 内存泄漏

**解决方案**:

1. 清理缓存:
   设置 → 缓存 → 清空

2. 清理历史记录:
   设置 → 历史 → 清空

3. 重启应用

---

#### Q: CPU 占用率高

**可能原因**:
1. Hook 监控频率过高
2. OCR 监控运行中
3. 剪贴板监听

**解决方案**:

1. 停止不需要的监控:
   - Hook 监控
   - OCR 监控
   - 剪贴板监听

2. 增加监控间隔:
   设置 → Hook → UIA 间隔 (默认 500ms)

---

#### Q: 翻译请求超时

**错误信息**: `TranslationError::NetworkError: request timeout`

**解决方案**:

1. 检查网络连接

2. 增加超时时间:
   设置 → 高级 → 请求超时

3. 使用代理

4. 尝试其他引擎

---

## 错误代码

### TranslationError 类型

| 错误类型 | 说明 | 解决方案 |
|----------|------|----------|
| `NoEngine` | 没有可用的翻译引擎 | 启用至少一个引擎 |
| `AllEnginesFailed` | 所有引擎都失败 | 检查网络和 API 配置 |
| `EngineError` | 单个引擎错误 | 检查该引擎配置 |
| `RateLimited` | 请求频率超限 | 等待或更换 API Key |
| `InvalidInput` | 输入无效 | 检查输入文本 |
| `ConfigError` | 配置错误 | 检查配置文件 |
| `NetworkError` | 网络错误 | 检查网络连接 |
| `CacheError` | 缓存错误 | 清空缓存 |
| `PluginError` | 插件错误 | 检查插件状态 |
| `StreamingNotSupported` | 不支持流式输出 | 使用非流式模式 |
| `Internal` | 内部错误 | 查看日志 |

### HTTP API 错误码

| 状态码 | 说明 |
|--------|------|
| 200 | 成功 |
| 400 | 请求参数错误 |
| 429 | 请求频率超限 |
| 500 | 服务器内部错误 |
| 503 | 服务不可用 (无可用引擎) |

---

## 调试技巧

### 1. 启用详细日志

设置环境变量:

```powershell
# PowerShell
$env:RUST_LOG="debug"

# CMD
set RUST_LOG=debug
```

日志级别: `error`, `warn`, `info`, `debug`, `trace`

---

### 2. 查看应用日志

日志文件位置:
```
%APPDATA%\moontranslator\logs\
```

实时查看日志:

```powershell
# PowerShell
Get-Content "$env:APPDATA\moontranslator\logs\app.log" -Wait
```

---

### 3. 使用开发者工具

1. 右键点击应用 → 检查元素
2. 或按 `F12` (如果可用)
3. 查看 Console 和 Network 选项卡

---

### 4. 测试 API 服务器

```bash
# 健康检查
curl http://127.0.0.1:60828/health

# 测试翻译
curl -X POST http://127.0.0.1:60828/translate \
  -H "Content-Type: application/json" \
  -d '{"text": "Hello", "from": "en", "to": "zh"}'

# 获取配置
curl http://127.0.0.1:60828/config

# 获取引擎列表
curl http://127.0.0.1:60828/engines
```

---

### 5. 检查配置文件

配置文件位置:
```
%APPDATA%\moontranslator\config.json
```

验证 JSON 格式:

```powershell
# PowerShell
Get-Content "$env:APPDATA\moontranslator\config.json" | ConvertFrom-Json
```

---

### 6. 重置配置

删除配置文件后重启应用:

```powershell
Remove-Item "$env:APPDATA\moontranslator\config.json"
```

---

### 7. 检查网络连接

```bash
# 测试 DeepSeek API
curl https://api.deepseek.com/v1/models

# 测试 Google 翻译
curl https://translate.googleapis.com/translate_a/single?client=gtx&sl=en&tl=zh&dt=t&q=Hello

# 测试代理 (如果配置了)
curl -x http://127.0.0.1:7890 https://api.deepseek.com/v1/models
```

---

## 日志分析

### 日志格式

```
2024-01-15T10:30:45.123Z  INFO moontranslator::engine::router: [Router] Using primary engine: LLM
2024-01-15T10:30:45.456Z DEBUG moontranslator::engine::llm: [LLM] Sending request to https://api.deepseek.com/v1/chat/completions
2024-01-15T10:30:46.789Z  INFO moontranslator::engine::llm: [LLM] Translation completed in 1333ms
```

### 常见日志模式

**引擎选择**:
```
INFO moontranslator::engine::router: [Router] Configured engines: ["LLM", "Google", "Youdao"] (strategy: FallbackOnError)
```

**翻译成功**:
```
INFO moontranslator::services::translation: Translation completed: "Hello" -> "你好" (Google, 150ms)
```

**翻译失败**:
```
WARN moontranslator::engine::router: [Router] Engine LLM failed: request timeout, trying next...
ERROR moontranslator::engine::router: [Router] All engines failed
```

**缓存命中**:
```
DEBUG moontranslator::cache: Cache hit for hash: abc123
```

**API 请求**:
```
INFO moontranslator::api_server: POST /translate 200 OK (150ms)
```

### 错误日志分析

**网络错误**:
```
ERROR moontranslator::engine::llm: [LLM] Network error: connection refused
```
→ 检查网络连接和代理设置

**API 错误**:
```
ERROR moontranslator::engine::llm: [LLM] API error: 401 Unauthorized
```
→ 检查 API Key

**配置错误**:
```
ERROR moontranslator::config: Failed to parse config file: invalid JSON
```
→ 检查配置文件格式

---

## 获取帮助

### 1. 检查已知问题

查看 GitHub Issues:
```
https://github.com/your-username/moontranslator/issues
```

### 2. 提交 Bug 报告

使用 Issue 模板提交:
```
https://github.com/your-username/moontranslator/issues/new?template=bug_report.md
```

**提供信息**:
- 操作系统版本
- 应用版本
- 错误信息
- 复现步骤
- 日志文件

### 3. 社区支持

- GitHub Discussions
- Discord (如果有)

### 4. 联系维护者

- Email: your-email@example.com

---

## 快速修复清单

遇到问题时，按顺序尝试:

1. [ ] 重启应用
2. [ ] 检查网络连接
3. [ ] 更新到最新版本
4. [ ] 清空缓存
5. [ ] 重置配置
6. [ ] 查看日志
7. [ ] 提交 Issue

---

## 已知问题

### Windows 特定

- 某些 Electron 应用 (如 VS Code) 的 UIA 文本获取可能不稳定
- 高 DPI 屏幕下截图区域可能偏移
- 管理员权限运行的应用可能无法获取文本

### 浏览器扩展

- 某些 CSP 严格的网站可能阻止 Content Script
- Shadow DOM 内的元素可能无法翻译
- 动态加载的内容需要手动触发翻译

### OCR

- 手写体识别准确率较低
- 复杂背景下的文字识别困难
- 特殊字体可能识别错误

---

## 性能优化建议

### 1. 减少内存占用

- 定期清理缓存
- 限制历史记录数量
- 关闭不需要的功能

### 2. 提高翻译速度

- 使用 `primary_only` 策略
- 配置快速引擎为首选
- 使用代理减少延迟

### 3. 降低 CPU 使用

- 增加监控间隔
- 停止不需要的后台任务
- 减少并发翻译数量

---

## 隐私和安全

### 数据存储

- 配置文件: `%APPDATA%\moontranslator\config.json`
- 历史记录: `%APPDATA%\moontranslator\history.db`
- 缓存: 内存中 (应用关闭后清除)

### 网络请求

- 翻译请求发送到配置的引擎 API
- API 服务器仅监听 localhost
- 不收集用户数据

### API 密钥安全

- 密钥存储在本地配置文件
- API 响应中密钥脱敏
- 不上传到任何服务器

---

*最后更新: 2024 年 1 月*
