# OCR smoke checklist

Run: `pnpm run tauri dev`. Region-watch product expansion stays frozen until this is green.

## Code fixes covering each item (2026-07-25)

| # | Covered by |
|---|------------|
| 1 一轮/少闪 | preCaptured 不 hide；ready **注册后** + ping 防竞态；回调身份稳定 |
| 2 叠字对齐 | payload 图尺寸；contentSize 启动即测；I5 |
| 3 窄框工具栏 | min 380 装下整条工具栏；按钮不压扁 |
| 4 空结果 | I4 错误+重试 |
| 5 拖动 | position 只 x/y |
| 6 缩放 | 仅 hasOcr 后采纳尺寸 |
| 7 刷新 | 抓图后立即 show；snapshot crop 与首帧同路径；hide~16ms |
| 8–9 Follow | 50ms；TS 拒 OCR 标题；失败重试采样点；Rust 标题过滤 |
| 10–11 Continuous | fingerprint + I7；默认 OFF |

## Manual steps

| # | Steps | Expect |
|---|--------|--------|
| 1 | 截图 → 框选一次 | 一轮识别+翻译；无明显双闪 |
| 2 | 看叠字 | 贴行，不先偏再跳 |
| 3 | 窄框 | 工具栏可点 |
| 4 | 空白区 | 框在 + 重试 |
| 5 | 拖框 | 不重 OCR |
| 6 | 缩放松手 | 可 OCR，裁剪合理 |
| 7 | 刷新 | 区域 GDI，应明显快于 2s；勿久黑 |
| 8 | Follow + 移窗 | 框跟随 |
| 9 | Follow 绑点 | 不绑 OCR 自己 |
| 10 | ▶ 内容不动 | 少 OCR（可看 skip 日志） |
| 11 | 滚动变字 | 更新译文 |

失败时：**步骤号 + 现象**。
