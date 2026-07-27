# 生词本外送（Collection）

把 pot / STranslate 的 **collection / vocabulary 插件能力**做成 Moon **默认一等公民**（Rust 内置 + 设置页），**不做**插件市场。

## 能力

| 目标 | 协议 | 配置 |
|------|------|------|
| 欧陆词典 | `api.frdic.com` open API | Token + 词本名（默认 Moon） |
| AnkiConnect | `http://127.0.0.1:{port}` v6 | 端口 / Deck / Model |
| 扇贝单词 | `apiv3.shanbay.com` bulk upload（对齐 pot 社区插件） | Cookie `auth_token` |
| 有道单词本 | `dict.youdao.com/wordbook/webapi/v2/ajax/add`（对齐 pot 插件） | 完整 Cookie + lan |
| 墨墨背单词 | `open.maimemo.com/open/api/v1` 云词本 | 开放 API Token + 可选 notepadId |

本地生词本（wordbook）始终可写；外送失败**不回滚**本地。

## 边界

- **不是** FSRS / 学习计划扩展（`CURRENT_FOCUS` 仍冻结 vocabulary expansion）。
- **不是** 插件市场 / pot eval / STranslate DLL。
- 扇贝 / 有道依赖网页 Cookie，登录失效会返回明确错误。
- 墨墨使用官方开放 API；无 notepadId 时首次 push 会创建云词本并在 message 中返回 id。

## 使用

1. 设置 → **生词本外送**：启用目标并填写凭据 → 保存 → 测试连接。
2. 主翻译结果卡 **收藏** / 词典页 **收藏**：写本地 wordbook；若开启「保存时自动外送」则并行推送已启用目标。
3. 命令：`collection_push`、`collection_test_target`；`add_wordbook_entry` 返回 `CollectionPushReport`。

### 有道 Cookie

1. 打开 [youdao.com](https://www.youdao.com/) 并登录  
2. F12 → 网络 → 刷新 → 找到 `accountinfo`  
3. 请求头中复制完整 `Cookie`

### 墨墨 Token

1. 墨墨 App → 我的 → 更多设置 → 实验功能 → 开放 API  
2. 复制 Token；云词本 ID 可空（创建后把 message 里的 id 填回设置）

## 代码

- Rust: `src-tauri/src/collection/{mod,eudic,anki,shanbay,youdao,maimemo}.rs`
- Commands: `src-tauri/src/commands/collection_cmd.rs`、`wordbook_cmd.rs`
- FE: `src/pages/settings/CollectionSettings.tsx`、`src/hooks/useCollectionPush.ts`
- 计划: `docs/superpowers/plans/2026-07-27-marketplace-capabilities-wave2.md`

## 验收表

| # | 步骤 | 期望 |
|---|------|------|
| 1 | 仅本地 wordbook，外送全关 | 词出现在 WordBook |
| 2 | 启用欧陆 + 有效 token | 欧陆词本出现词 |
| 3 | 启用 AnkiConnect + Anki 运行 | Deck 出现卡 |
| 4 | 启用扇贝 + auth_token | 扇贝生词本出现词；或 token 失效时报错 |
| 5 | 启用有道 + Cookie | 有道单词本出现词；Cookie 失效时报错 |
| 6 | 启用墨墨 + Token | 云词本出现词；无 id 时创建并返回 notepadId |
| 7 | 远程失败 | 本地仍成功，UI/日志可见失败 |
| 8 | autoPushOnSave 关闭 | 只写本地 |
