# Moon Translator 浏览器扩展

强大的翻译浏览器扩展，支持 Chrome 和 Firefox。

## 功能特性

- **划词翻译** - 选中文本后自动显示翻译结果
- **整页翻译** - 一键翻译整个网页
- **多引擎支持** - Google、有道、Microsoft、LLM、DeepL
- **右键菜单** - 选中文本右键翻译
- **快捷键** - Alt+T 快速翻译选中文本
- **暗色模式** - 自动跟随系统主题

## 安装

### Chrome

```bash
node build.js
```

1. 打开 `chrome://extensions/`
2. 开启「开发者模式」
3. 点击「加载已解压的扩展程序」
4. 选择 `extension/dist/chrome` 文件夹

### Firefox

```bash
node build.js
```

1. 打开 `about:debugging#/runtime/this-firefox`
2. 点击「临时载入附加组件」
3. 选择 `extension/dist/firefox/manifest.json`

### 构建发布版本

```bash
cd extension
node build.js
```

构建产物目录为 `dist/chrome/` 和 `dist/firefox/`，同时生成 `.tar.gz` 归档。

## 翻译引擎

| 引擎 | 需要 API Key | 说明 |
|------|-------------|------|
| Google | 否 | 免费，稳定 |
| 有道 | 否 | 免费，支持中文优化 |
| Microsoft | 否 | 免费，Edge 浏览器内置 |
| LLM | 是 | 支持 DeepSeek、OpenAI 等 |
| DeepL | 是 | 高质量翻译 |

## 配置

点击扩展图标打开设置面板，可以：

- 启用/禁用翻译引擎
- 配置 LLM API Key
- 设置默认源语言和目标语言
- 配置 DeepL API
- 粘贴桌面桥接 API 令牌（`desktopApiToken`）

## 桌面桥接（可选）

扩展默认连接桌面本地 API：`http://127.0.0.1:60828`。桌面 **API 服务器默认关闭**。

1. 打开桌面版 → **高级设置 → 浏览器扩展**
2. 启用「桌面桥接服务 / API 服务器」（`apiServerEnabled`）
3. 确认端口（默认 `60828`，`apiServerPort`）
4. 复制 **API 令牌**（`apiServerToken`）到扩展弹窗「桌面桥接令牌」
5. **重启桌面应用** 使监听生效

鉴权：除 `GET /health` 外，请求需带：

```http
Authorization: Bearer <apiServerToken>
```

或 `X-Api-Token: <apiServerToken>`。

控制路由（需鉴权，触发桌面动作）：

| 方法 | 路径 | 作用 |
|------|------|------|
| POST | `/control/show` | 显示主窗口 |
| POST | `/control/selection_translate` | 划词翻译 |
| POST | `/control/ocr_translate` | OCR 截图翻译 |
| POST | `/control/open_settings` | 打开设置 |

桥接不可用时弹窗会提示启用 API 服务器；扩展回退到本地引擎。完整接口见仓库 `docs/API.md`。

## 开发

```
extension/
├── manifest.json          # 扩展清单
├── background/
│   └── service-worker.js  # 后台服务
├── content/
│   ├── selector.js        # 划词翻译
│   ├── selector.css       # 样式
│   └── page-translator.js # 整页翻译
├── popup/
│   ├── popup.html         # 弹出窗口
│   ├── popup.js           # 弹出窗口逻辑
│   └── popup.css          # 弹出窗口样式
├── icons/                 # 扩展图标
├── build.js               # 构建脚本
└── README.md              # 本文件
```

## 许可证

MIT License
