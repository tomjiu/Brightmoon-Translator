# 引擎扩展计划 - 有道OCR修复 + 新引擎集成

## 一、有道OCR修复方案

### 可以修复吗？✅ 可以

#### 方案1: 使用有道智云OCR API（官方付费API）

**API文档**: https://ai.youdao.com/DOCSIRMA/html/ocr/api/zyocr/index.html

**端点**: 
```
https://openapi.youdao.com/ocrapi
```

**认证方式**:
```rust
// 需要签名
use sha2::{Sha256, Digest};
use chrono::Utc;

fn generate_sign(
    app_key: &str,
    q: &str,  // 图片内容截取（前10个字符）
    salt: &str,
    curtime: &str,
    app_secret: &str,
) -> String {
    let sign_str = format!("{}{}{}{}{}", app_key, q, salt, curtime, app_secret);
    let mut hasher = Sha256::new();
    hasher.update(sign_str.as_bytes());
    format!("{:x}", hasher.finalize())
}

// 请求参数
let params = [
    ("img", base64_image),
    ("langType", "auto"),
    ("detectType", "10012"),  // 通用文字识别
    ("imageType", "1"),       // base64
    ("appKey", app_key),
    ("salt", &uuid::Uuid::new_v4().to_string()),
    ("curtime", &Utc::now().timestamp().to_string()),
    ("sign", &generated_sign),
    ("signType", "v3"),
];
```

**费用**: 
- 免费额度: 每月50次
- 付费: ¥0.001/次起

**优势**:
- ✅ 官方API，稳定可靠
- ✅ 识别质量好
- ✅ 支持多语言

**劣势**:
- ❌ 需要API key
- ❌ 有限免费额度

---

#### 方案2: 使用旧版有道词典API（逆向）

**端点**（你代码中已有的密钥对应的API）:
```
旧端点: https://ocrtran.youdao.com/ocrtranapi (404)
可能的新端点: 需要抓包有道词典找到
```

**内置密钥**:
```rust
// 有道词典的内置密钥（已在代码中）
app_key: "3d9fa94028675971"
app_secret: "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p"
```

**如何找到新端点**:
1. 安装有道词典
2. 使用Fiddler/Charles抓包
3. 截图翻译，查看API调用
4. 复制新的端点和参数格式

**优势**:
- ✅ 无需用户配置API key
- ✅ 免费

**劣势**:
- ⚠️ 非官方，可能随时失效
- ⚠️ 需要逆向工程

**推荐**: 先尝试方案1（官方API），让用户自行配置key

---

## 二、更多翻译引擎推荐

### 1. 🌟 彩云小译（Caiyun Translate）- 推荐

**官网**: https://fanyi.caiyunapp.com/  
**API**: https://docs.caiyunapp.com/blog/2018/09/03/lingocloud-api/

**特点**:
- ✅ **擅长上下文翻译**（小说、文章）
- ✅ 支持中英、中日互译
- ✅ 免费额度：100万字/月
- ✅ 质量好，适合长文本

**集成代码**:
```rust
// src-tauri/src/engine/caiyun.rs

pub struct CaiyunEngine {
    api_token: String,
}

impl TranslationEngine for CaiyunEngine {
    async fn translate(&self, text: &str, from: &str, to: &str) -> Result<String> {
        let client = reqwest::Client::new();
        
        let payload = json!({
            "source": vec![text],
            "trans_type": format!("{}2{}", from, to),  // "auto2zh"
            "request_id": uuid::Uuid::new_v4().to_string(),
        });
        
        let resp = client
            .post("https://api.interpreter.caiyunai.com/v1/translator")
            .header("X-Authorization", format!("token {}", self.api_token))
            .json(&payload)
            .send()
            .await?;
        
        let data: serde_json::Value = resp.json().await?;
        let result = data["target"][0].as_str().unwrap_or(text);
        
        Ok(result.to_string())
    }
}
```

**费用**: 
- 免费: 100万字/月
- 付费: ¥20/百万字

---

### 2. 火山翻译（Volcengine）

**官网**: https://www.volcengine.com/products/machine-translation  
**字节跳动出品**

**特点**:
- ✅ 字节跳动技术
- ✅ 质量不错
- ✅ 支持多语言
- ✅ 免费额度：200万字/月

**费用**:
- 免费: 200万字/月
- 付费: ¥28/百万字

---

### 3. 腾讯翻译君（Tencent）

**官网**: https://cloud.tencent.com/product/tmt

**特点**:
- ✅ 大厂出品
- ✅ 稳定可靠
- ✅ 免费额度：500万字/月

**费用**:
- 免费: 500万字/月
- 付费: ¥58/百万字

---

### 4. 阿里翻译（Alibaba）

**官网**: https://www.aliyun.com/product/ai/alimt

**特点**:
- ✅ 电商翻译准确
- ✅ 支持专业领域
- ✅ 免费额度：100万字/月

**费用**:
- 免费: 100万字/月
- 付费: ¥50/百万字

---

### 5. OpenL（开源翻译）

**GitHub**: https://github.com/LibreTranslate/LibreTranslate

**特点**:
- ✅ **完全开源**
- ✅ **可本地部署**
- ✅ 无API调用限制
- ✅ 支持30+语言

**劣势**:
- ⚠️ 质量略逊于商业API
- ⚠️ 需要服务器部署

---

## 三、更好的OCR引擎

### 1. 🌟 百度OCR（推荐）

**官网**: https://ai.baidu.com/tech/ocr

**特点**:
- ✅ **识别准确率高**
- ✅ 支持多种场景（通用、手写、表格等）
- ✅ 免费额度：1000次/天
- ✅ 价格便宜

**API示例**:
```rust
// src-tauri/src/ocr/baidu.rs

pub async fn baidu_ocr(
    image_base64: &str,
    access_token: &str,
) -> Result<OcrResultDetailed> {
    let client = reqwest::Client::new();
    
    let params = [
        ("image", image_base64),
        ("language_type", "auto_detect"),
        ("detect_direction", "true"),
        ("paragraph", "true"),
        ("probability", "true"),
    ];
    
    let resp = client
        .post(format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/accurate_basic?access_token={}",
            access_token
        ))
        .form(&params)
        .send()
        .await?;
    
    // 解析结果...
    parse_baidu_ocr_response(resp).await
}
```

**费用**:
- 免费: 1000次/天
- 付费: ¥0.002/次

---

### 2. 腾讯OCR

**官网**: https://cloud.tencent.com/product/ocr

**特点**:
- ✅ 识别准确
- ✅ 免费额度：1000次/月
- ✅ 支持各种证件、票据

**费用**:
- 免费: 1000次/月
- 付费: ¥0.0015/次

---

### 3. 阿里OCR

**官网**: https://www.aliyun.com/product/ocr

**特点**:
- ✅ 多场景支持
- ✅ 免费额度：500次/月

**费用**:
- 免费: 500次/月
- 付费: ¥0.005/次

---

### 4. 🌟 PaddleOCR（本地部署，推荐）

**GitHub**: https://github.com/PaddlePaddle/PaddleOCR

**特点**:
- ✅ **完全开源**
- ✅ **可本地运行**
- ✅ **超高准确率**（与百度OCR同源技术）
- ✅ 支持80+语言
- ✅ 轻量级模型（8.6MB）
- ✅ CPU可运行

**Rust集成方案**:

#### 方案A: Python sidecar
```rust
// 启动PaddleOCR Python服务
use tauri::api::process::{Command, CommandEvent};

let (mut rx, _child) = Command::new_sidecar("paddleocr_server")
    .expect("Failed to create sidecar")
    .spawn()
    .expect("Failed to spawn sidecar");

// HTTP调用
let client = reqwest::Client::new();
let resp = client
    .post("http://localhost:8866/predict/ocr_system")
    .json(&json!({
        "images": [base64_image],
    }))
    .send()
    .await?;
```

#### 方案B: ONNX Runtime（推荐）
```toml
[dependencies]
ort = "2.0"  # ONNX Runtime
```

```rust
// 加载PaddleOCR的ONNX模型
use ort::{Session, Value};

let session = Session::builder()?
    .with_model_from_file("models/paddle_ocr.onnx")?;

let input = prepare_image(image)?;
let outputs = session.run(vec![Value::from_array(input)?])?;
let text = decode_paddle_output(&outputs)?;
```

**优势**:
- ✅ 完全离线
- ✅ 无API限制
- ✅ 识别质量顶级
- ✅ 速度快（CPU ~200ms，GPU ~50ms）

---

## 四、推荐的引擎组合

### 翻译引擎优先级

```
Tier 1 (免费优先):
1. Google（免费，质量好）
2. Youdao（免费，速度快）
3. OpenL（开源，本地）

Tier 2 (免费额度大):
4. 彩云小译（100万字/月，长文本好） ⭐
5. 腾讯翻译（500万字/月）
6. 火山翻译（200万字/月）

Tier 3 (付费可选):
7. DeepL（质量最好）
8. Baidu（需API key）
```

### OCR引擎优先级

```
Tier 1 (主力):
1. WinRT OCR（系统原生，快速） ✅
2. PaddleOCR（本地部署，准确率高） ⭐

Tier 2 (云端备用):
3. 百度OCR（1000次/天免费，准确）
4. 腾讯OCR（1000次/月免费）

Tier 3 (最后备用):
5. Tesseract.js（浏览器，离线）
6. 有道OCR（如果修复）
```

---

## 五、实施计划

### Phase 1: 修复和补全（1周）

#### Day 1-2: 修复有道OCR
- [ ] 尝试官方API（需用户配置key）
- [ ] 或抓包找新端点（逆向）
- [ ] 测试验证

#### Day 3-4: 集成彩云小译 ⭐
- [ ] 实现CaiyunEngine
- [ ] 添加到配置页面
- [ ] 测试长文本翻译

#### Day 5: 集成百度OCR ⭐
- [ ] 实现BaiduOCR
- [ ] 添加配置选项
- [ ] 测试识别准确率

### Phase 2: 本地引擎（1-2周）

#### Week 2: PaddleOCR集成 ⭐
- [ ] 下载ONNX模型
- [ ] 集成ort运行时
- [ ] 优化推理速度
- [ ] 作为默认OCR

#### Week 3: OpenL本地翻译（可选）
- [ ] 集成LibreTranslate
- [ ] 本地模型部署
- [ ] 作为离线备用

---

## 六、配置界面设计

### 翻译引擎配置
```typescript
interface EngineConfig {
  // 免费引擎
  google: { enabled: boolean };
  youdao: { enabled: boolean };
  
  // 需要配置的免费额度引擎
  caiyun: {
    enabled: boolean;
    apiToken: string;  // 用户申请
  };
  tencent: {
    enabled: boolean;
    secretId: string;
    secretKey: string;
  };
  
  // 付费引擎
  deepl: {
    enabled: boolean;
    apiKey: string;
    pro: boolean;
  };
  
  // 本地引擎
  openl: {
    enabled: boolean;
    endpoint: string;  // 默认localhost:5000
  };
}
```

### OCR引擎配置
```typescript
interface OcrConfig {
  // 本地引擎
  winrt: { enabled: boolean; priority: number };
  paddleocr: { enabled: boolean; priority: number };
  
  // 云端引擎
  baidu: {
    enabled: boolean;
    apiKey: string;
    secretKey: string;
  };
  tencent: {
    enabled: boolean;
    secretId: string;
    secretKey: string;
  };
  youdao: {
    enabled: boolean;
    appKey: string;
    appSecret: string;
  };
}
```

---

## 七、立即可做的Quick Wins

### 1. 集成彩云小译（4小时）

**优先级**: P1  
**理由**: 免费额度大，质量好，适合我们的场景

**步骤**:
1. 添加 `src-tauri/src/engine/caiyun.rs`（1小时）
2. 注册到Router（30分钟）
3. 添加前端配置UI（1小时）
4. 测试验证（1小时）

### 2. 集成百度OCR（4小时）

**优先级**: P1  
**理由**: 识别准确率高，免费额度够用

**步骤**:
1. 添加 `src-tauri/src/ocr/baidu.rs`（1小时）
2. 实现access_token获取（30分钟）
3. 添加前端配置UI（1小时）
4. 测试验证（1小时）

### 3. 修复有道OCR（2-3小时）

**优先级**: P2（可选）  
**方法**: 使用官方API + 用户配置key

---

## 总结

### 可以修复吗？
✅ **有道OCR**: 可以修复，但需要用户配置API key

### 有更好的选择吗？
✅ **是的！**
- **翻译**: 彩云小译（免费额度大）
- **OCR**: PaddleOCR（本地，准确率高）+ 百度OCR（云端备用）

### 推荐行动
1. **优先**: 集成彩云小译 + 百度OCR
2. **其次**: PaddleOCR本地部署
3. **可选**: 修复有道OCR

**总工作量**: 约2周

需要我开始实施吗？建议从彩云小译开始！

---

**文档日期**: 2026-06-12  
**分析者**: Claude Opus 4.8 (1M context)  
**推荐优先级**: 彩云小译(P1) > 百度OCR(P1) > PaddleOCR(P0)
