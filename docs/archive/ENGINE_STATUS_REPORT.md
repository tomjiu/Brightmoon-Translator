# 翻译引擎状态梳理 - 2026-06-12

## 当前引擎状态概览

### 翻译引擎

#### ✅ 正常工作
1. **Youdao（有道翻译）** - 文本翻译
   - API: `dict-trans.youdao.com`
   - 状态: ✅ 正常
   - 无需API key
   - 免费公共API

2. **Google翻译**
   - 状态: ✅ 正常
   - 无需API key
   - 免费公共API

3. **DeepL** (需配置)
   - 状态: ⚠️ 需要API key
   - 用户自行配置

4. **Baidu（百度翻译）** (需配置)
   - 状态: ⚠️ 需要API key
   - 用户自行配置

---

### OCR引擎

#### ✅ 正常工作
1. **WinRT OCR** (Windows系统OCR)
   - 状态: ✅ 完全正常
   - 识别质量: 良好
   - 性能: 快速
   - 支持: 中文、英文等
   - **这是主要的OCR引擎**

#### ⚠️ 有问题但有Fallback
2. **Youdao OCR（有道OCR）**
   - API端点: `https://ocrtran.youdao.com/ocrtranapi`
   - 状态: ⚠️ **返回404**
   - 原因分析: 
     - API端点可能已变更或废弃
     - 免费额度可能已用完
     - 需要新的认证方式
   - **影响**: ❌ 无影响！
     - 因为有WinRT OCR作为主引擎
     - Youdao OCR是备用fallback
     - 应用会自动跳过失败的引擎

#### ✅ 可用的备用方案
3. **Tesseract.js** (浏览器模式)
   - 状态: ✅ 可用
   - 用途: 浏览器扩展中使用
   - 性能: 较慢，离线可用

---

## 实际运行时的引擎选择逻辑

### OCR流程（桌面版）
```rust
// src/services/ocr.ts - ocrImagePreferNativeDetailed()

async function ocrImagePreferNativeDetailed(imageDataUrl, lang) {
    // 1. 并行执行WinRT和Youdao
    const winrtPromise = ocrImageDetailed(imageDataUrl, lang);
    const youdaoPromise = youdaoOcrDetailed(imageDataUrl, lang);
    
    // 2. 谁先成功就用谁（Race策略）
    const firstSuccess = await Promise.race([
        winrtPromise.catch(() => null),
        youdaoPromise.catch(() => null),
    ]);
    
    if (firstSuccess) {
        return firstSuccess;  // ✅ 通常是WinRT先成功
    }
    
    // 3. Fallback to tesseract.js
    return await ocrImageTesseract(imageDataUrl);
}
```

### 实际测试结果（从日志）
```
2026-06-12T09:49:30 [INFO] [WinRT OCR] Success: 6 lines, 314 chars
2026-06-12T09:49:31 [INFO] [Youdao OCR] Response status: 404 Not Found
                                         ↑ 但不影响，因为WinRT已成功
```

**结论**: 
- ✅ WinRT OCR正常工作，识别成功
- ❌ Youdao OCR返回404，但不影响功能
- ✅ 用户完全可以正常使用OCR翻译

---

## Youdao OCR 404 问题详细分析

### 当前实现
```rust
// src-tauri/src/commands/capture.rs:1116
let endpoint = "https://ocrtran.youdao.com/ocrtranapi";

// 请求参数
let form = multipart::Form::new()
    .text("img", image_base64)
    .text("lang", lang_from)
    .text("type", "1")
    .text("docType", "json");

// 结果: 404 Not Found
```

### 可能的原因

#### 原因1: API端点变更 ⭐ 最可能
有道可能更换了API地址：
- 旧端点: `ocrtran.youdao.com/ocrtranapi` ❌
- 新端点: 可能需要查找

#### 原因2: 需要认证
免费公开API可能已关闭，现在需要：
- App Key
- App Secret
- 签名认证

#### 原因3: 地域限制
API可能有地域限制或被墙

---

## 解决方案

### 方案1: 保持现状 ✅ 推荐

**理由**:
- WinRT OCR完全够用
- 识别质量好，速度快
- 系统原生，无需网络
- 用户无感知

**操作**: 无需任何修改

### 方案2: 修复Youdao OCR（可选）

#### Step 1: 使用有道官方OCR API

**新的API端点**:
```
https://openapi.youdao.com/ocrapi
```

**需要配置**:
```rust
// 在config.rs添加
pub struct YoudaoConfig {
    pub enabled: bool,
    pub use_ai: bool,
    pub ocr_app_key: String,     // ← 新增
    pub ocr_app_secret: String,  // ← 新增
}
```

**签名算法**:
```rust
use sha2::{Sha256, Digest};

fn generate_youdao_sign(
    app_key: &str,
    app_secret: &str,
    q: &str,
    salt: &str,
    curtime: &str,
) -> String {
    let sign_str = format!("{}{}{}{}{}", app_key, q, salt, curtime, app_secret);
    let mut hasher = Sha256::new();
    hasher.update(sign_str.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

**参考文档**:
- https://ai.youdao.com/DOCSIRMA/html/ocr/api/zyocr/index.html

#### Step 2: 或者使用内置密钥

你的代码中已经有内置密钥：
```rust
// src-tauri/src/models/config.rs:84
fn default_youdao_ocr_app_key() -> String {
    "3d9fa94028675971".to_string()
}

fn default_youdao_ocr_app_secret() -> String {
    "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p".to_string()
}
```

这是**有道词典的内置密钥**，可以尝试使用。

### 方案3: 移除Youdao OCR

如果不需要，可以完全移除：
```typescript
// src/services/ocr.ts
export async function ocrImageDetailed(imageDataUrl, lang) {
    // 只使用WinRT OCR
    return await ocrImageWinRT(imageDataUrl, lang);
}
```

---

## 配置中的问题

### 当前配置结构
```rust
// src-tauri/src/models/config.rs
pub struct YoudaoConfig {
    pub enabled: bool,
    pub use_ai: bool,
    // OCR密钥
    pub ocr_app_key: String,
    pub ocr_app_secret: String,
}
```

**问题**: OCR密钥和翻译混在一起

### 建议改进

```rust
pub struct YoudaoConfig {
    // 翻译相关
    pub enabled: bool,
    pub use_ai: bool,
    
    // OCR相关（分离）
    pub ocr_enabled: bool,
    pub ocr_app_key: String,
    pub ocr_app_secret: String,
}
```

---

## 测试验证

### 测试WinRT OCR是否正常
```bash
# 启动应用
npm run tauri dev

# 点击OCR按钮
# 选择区域
# 查看日志
```

**预期日志**:
```
[INFO] [WinRT OCR] Starting detailed OCR recognition
[INFO] [WinRT OCR] Success: X lines, Y chars total
[WARN] [Youdao OCR] Response status: 404 Not Found  ← 可以忽略
```

### 测试Youdao OCR修复（如果修复）
```rust
// 修改后测试
cargo test youdao_ocr_with_key

// 或手动测试
#[tokio::test]
async fn test_youdao_ocr() {
    let result = youdao_ocr_with_credentials(
        "test.png",
        "3d9fa94028675971",
        "5X2CJlMERfGOkOP0PFqokVJkSgDIOD0p",
    ).await;
    
    assert!(result.is_ok());
}
```

---

## 推荐行动

### 立即行动（如果需要）

#### 选项A: 保持现状 ✅
- **不做任何修改**
- WinRT OCR工作正常
- Youdao OCR 404不影响使用
- 用户无感知

#### 选项B: 尝试修复Youdao OCR
1. 使用内置密钥测试新API端点
2. 添加签名算法
3. 测试是否可用
4. 如果可用，更新代码

**工作量**: 2-3小时

#### 选项C: 完全移除Youdao OCR
- 简化代码
- 只保留WinRT OCR
- 减少复杂度

**工作量**: 1小时

---

## 总结

### 当前状态
- ✅ **OCR功能正常**（WinRT工作）
- ⚠️ **Youdao OCR 404**（不影响使用）
- ✅ **翻译引擎正常**（Youdao翻译、Google等）

### 问题严重程度
- **级别**: 低（Low Priority）
- **影响**: 无（有Fallback）
- **用户感知**: 无

### 建议
1. **不紧急**: 保持现状即可
2. **可选**: 有时间可以尝试修复
3. **替代**: 考虑使用其他OCR API（如百度OCR、腾讯OCR）

---

**你想怎么处理？**
1. 保持现状（WinRT OCR够用）
2. 修复Youdao OCR（尝试新API）
3. 完全移除Youdao OCR（简化代码）
4. 集成其他OCR API（如百度、腾讯）

---

**文档日期**: 2026-06-12  
**分析者**: Claude Opus 4.8 (1M context)  
**状态**: Youdao OCR 404，但不影响功能
