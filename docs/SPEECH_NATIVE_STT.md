# 语音识别：原生 STT(Windows.Media.SpeechRecognition)改造方案

> 状态：**方案(调研完成,未改代码)** · 范围：主页语音按钮
> 关联：`src/hooks/useSpeechRecognition.ts` · `src-tauri/src/speech.rs` · `src-tauri/src/commands/speech_cmd.rs` · `src/pages/MainTranslator.tsx`

## 1. 问题根因

主页"语音"按钮(`MainTranslator.tsx`,麦图标)依赖 **Web Speech API**
(`useSpeechRecognition.ts` → `window.SpeechRecognition / webkitSpeechRecognition`)：

1. 应用跑在 **Tauri WebView2**(Chromium shell)里。Google 已停止向非 Chrome/Edge
   shell 提供 Web Speech 识别服务——API 对象在 WebView2 中**存在**(`isSupported=true`,
   按钮得以显示),但底层服务失效,`start()` 后完全无结果。
2. Rust 侧其实预置了原生命令(`start_speech_recognition` / `stop_speech_recognition` /
   `get_speech_recognition_status` / `get_speech_languages`,已于 `lib.rs` 注册),但
   `speech.rs` 是**空壳**：只翻转 `SpeechState.is_listening`,不进行真实识别;注释也
   写明"识别靠前端 Web Speech API"。
3. 前端从未 `invoke` 这些命令,也没有识别结果事件流。

结论：**没有任何一条链路能做真实识别**。不是参数/配置问题,是架构上把识别能力
错误地寄托在了 WebView2 不支持的 Web Speech API 上。

## 2. 目标

- 主页语音按钮在 **Windows 原生环境下真正可用**（系统听写）。
- 走 WinRT `Windows.Media.SpeechRecognition`,复用现有 Tauri 命令 + 状态结构。
- 保留前端 `useSpeechRecognition` 现有接口签名,`MainTranslator.tsx` 尽量少改。

## 3. 技术选型

| 方案 | 结论 |
|---|---|
| **WinRT `Windows.Media.SpeechRecognition`**(推荐) | 与现有 WinRT 用法一致(capture.rs 已用 `Media::Ocr::OcrEngine`),`windows` crate 0.58 已引入,仅需加 feature;系统级听写,零额外依赖。 |
| SAPI `ISpRecognizer`(COM) | 更底层,需手写 COM 事件泵,复杂且旧;不选。 |
| vosk / whisper.onnx | 真离线,但体积大、CI 编译长,且需模型分发;本方案不引入(见 §8 备选)。 |

## 4. 依赖与权限改动

### 4.1 Cargo.toml

`[dependencies.windows]` features 增加识别相关命名空间(0.58 风格,与 `Media_Ocr` 一致)：

```toml
features = [
  # ... 现有 ...
  "Media_Ocr",
  "Speech_Recognition",   # ← 新增
  "Storage_Streams",
  # ... 现有(其他字段不动) ...
]
```

若实现角度需要,还可能要 `Foundation_TypedEventHandler`(事件订阅)。迁移时以
编译器报缺的 feature 为准逐个补。

### 4.2 麦克风权限(重点 ⚠️)

- 结论（实测决议）：本应用以 **NSIS/MSI 非打包(win32)桌面应用**分发,不使用
  appx/MSIX。`microphone` capability 属于 MSIX 包级声明,**对非打包 exe 不生效**
  也无需声明。桌面应用访问麦克风由系统「设置 → 隐私和安全性 → 麦克风 →
  允许桌面应用访问麦克风」控制,无需在自定义 `tests.manifest` 中添加任何节点
  （当前该 manifest 仅含 comctl32 v6 依赖,保持不变）。
- 若未来引入 MSIX/Store 打包,才需要在 appx manifest 声明 `microphone`
  capability；本里程碑不做。
- 无权限时 `SpeechRecognizer::Create` / `RecognizeAsync` 会抛访问拒绝错误,
  已映射到现有 `speech.micDenied` 文案,并提示用户在 Windows 设置 → 隐私 →
  麦克风 授权（详见 §5.3 事件协议与 §6 前端处理）。

## 5. Rust 侧实现要点

文件：`src-tauri/src/speech.rs`（重写）+ `src-tauri/src/commands/speech_cmd.rs`（微调）。

### 5.1 状态

沿用现有 `SpeechState`(`is_listening` / `language`),但语义升级为"识别会话活跃"。
`SpeechRecognitionResult{ text, confidence, is_final }` 与
`SpeechRecognitionStatus{ is_listening, language, error }` 保留,FE 契约不变。

### 5.2 识别生命周期

```
start_recognition(lang)
  ├─ SpeechRecognizer::new()
  ├─ set Language(lang_to_locale(lang))
  ├─ Constraints = SpeechRecognitionListConstraint / 听写约束
  ├─ CompileConstraintsAsync().await
  └─ 启动连续识别循环:
       RecognizeAsync().await → 结果经 Tauri emit 推给前端 → 循环(until stopped)

stop_recognition
  └─ StopRecognitionAsync() + 复位状态
```

实现选型(二选一):

- **A. 单发连续循环**(推荐)：一次只认一句,结束后用新的 `RecognizeAsync` 续听;
  简单、稳定,SpeechRecognizer 每次复用同一实例即可。
- **B. 自启动识别**(`SpeechContinuousRecognitionSession`):需要 `ResultGenerated`
  事件订阅 + `Foundation_TypedEventHandler`,能拿中间结果 `result.alternatives`,
  更接近现在 FE 的 `interimTranscript` 体验;实现成本略高。

> 推荐先做 A(逐句),interim 体验可直接把逐句结果先标 `is_final=false` 显示、
> 句终再标 `true`;后续再迭代 B。

### 5.3 事件协议(前端收流)

- 事件名：`speech-recognition-result`
- payload：`SpeechRecognitionResult`(camelCase json：`text` / `confidence` / `isFinal`)
- 另发 `speech-recognition-start` / `speech-recognition-stop` / `speech-recognition-error`
  同步状态(`error` 复用 `speech.*` i18n 文案)。

```jsonc
// speech-recognition-result
{ "text": "你好", "confidence": 0.92, "isFinal": true }
```

### 5.4 注意事项

- WinRT 调用在 `async fn` tauri command 内直接 `.await`(windows crate 提供)
  即可,无需 UI 线程;若需 COM 线程初始化参照 capture.rs OCR 路径。
- 语言模型依赖系统「语言 + 语音识别」功能包:用户未装对应语言包时
  `CompileConstraints` 失败 → 映射 `speech.error`/`micDenied` 文案,不崩溃。
- 状态线程安全沿用 `Arc<Mutex<SpeechState>>`;识别 session 句柄存入 `SpeechState`
  或 `OnceLock`,stop 时取用。

## 6. 前端改动要点

文件：`src/hooks/useSpeechRecognition.ts`(主要)+ `src/pages/MainTranslator.tsx`(微调)。

### 6.1 接口(保持不变,避免大改)

保留 `isListening / interimTranscript / error / startListening / stopListening /
isSupported / consumeTranscript`;实现从"Web Speech API"整体替换为：

```
startListening(lang)
  ├─ isSupported = isTauri(始终 true;可再查 get_speech_languages 非空)
  ├─ invoke('start_speech_recognition', { lang })
  └─ listen('speech-recognition-result') 续流 → 写入 accumulated/interim

stopListening()
  └─ invoke('stop_speech_recognition')
```

- `isListening` 由 `speech-recognition-start/stop` 事件驱动(而非本地乐观翻转),
  避免按钮状态与后端脱节。
- `consumeTranscript`/定时器拼接逻辑(300ms 轮询)保留,`MainTranslator.tsx:113`
  附近逻辑不动或微调。
- `error` 由 `speech-recognition-error` 事件写入现有文案键。

### 6.2 退化策略

- WebSpeech 探测仍保留:`window.SpeechRecognition` 存在时优先用(万一用户跑在
  真浏览器开发模式),否则走原生 invoke。二者共用同一套外部接口。
- 非 Tauri(browser dev)下按钮行为不回归,均有兜底文案。

## 7. 验证路径(遵守 AGENTS.md)

```powershell
# 本地快速验证(不做长 build)
cargo check            # Rust 编译/feature 检查
pnpm exec tsc --noEmit
pnpm exec vitest run
npx eslint             # 0 错误
```
- 更新/新增 `useSpeechRecognition` 单测：mock invoke + listen,断言结果流写入。
- 手动烟雾：Windows 设置确认麦克风授权 → 点按钮 → 说话 → 文本进源文本框。
- 发布构建走云 CI(推 tag),本机不跑 `pnpm tauri build`。

## 8. 备选 / 后续增强

- **连续识别中间结果(B 方案)**：`SpeechContinuousRecognitionSession` + 事件订阅,
  提供真正的实时 interim 体验;作为二期。
- **真正离线(vosk / whisper.onnx)**：如需完全无网无系统语言包,另行设计模型下载
  与推理管线,CI 构建时间显著增加,不在本期范围。
- **系统语言包缺失引导**：从 `get_speech_languages` 返回后,前端可提示用户安装
  对应语言包(Windows 设置 → 语言)。

## 9. 待办清单(实施顺序)

- [x] Cargo.toml:加 `Speech_Recognition`(+ 事件所需)feature
- [x] tauri.conf.json / manifest:非打包桌面应用无需 `microphone` capability(已决议,§4.2)
- [x] speech.rs:实现 start/stop 真实识别 + 事件 emit;保留 S/D struct
- [x] speech_cmd.rs:命令透传 + `AppHandle` 注入,确保 `lang` 走 `lang_to_locale`
- [x] useSpeechRecognition.ts:切原生 + 事件流;保留 WebSpeech 探测兜底
- [x] MainTranslator.tsx:仅在必要时微调(接口未变,无需改动)
- [ ] 单测更新 + 手动烟雾
- [ ] 本地检查(cargo check/tsc/vitest/eslint)全绿后走云 CI