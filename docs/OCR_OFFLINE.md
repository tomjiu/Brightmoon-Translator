# 离线 OCR（Rapid / Paddle）

Moon **不**把 ONNX/exe 打进安装包。选择 OCR 引擎为 `rapid` 或 `paddle` 后，在设置里填写 **pluginDir**。

## RapidOCR（推荐对齐 pot）

1. 下载 [pot-app-recognize-plugin-rapid](https://github.com/pot-app/pot-app-recognize-plugin-rapid) 对应平台包  
2. 解压，目录中应有 `RapidOcrOnnx.exe`（或 Unix 二进制）与 `models/`  
3. 设置 → OCR → RapidOCR → pluginDir 指到该目录  

## PaddleOCR-json（Windows）

1. 获取 [PaddleOCR-json](https://github.com/hiroi-sora/PaddleOCR-json) 发布包  
2. 目录含 `PaddleOCR-json.exe` 与 `models/config_ch.txt` 等  
3. OCR 引擎选 Paddle，pluginDir 指到该目录  

## 行为

- 子进程识别，返回纯文本（框选为整图一行，后续可增强）  
- 路径错误或 exe 缺失时返回可读错误，不崩溃  
