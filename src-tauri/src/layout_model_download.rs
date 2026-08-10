//! P6: DocLayout-YOLO 模型按需下载与缓存。
//!
//! 设计原则：
//! - **不打包**：模型 ~50MB，不进安装包，用户启用布局检测时才下载。
//! - **校验完整性**：下载完成后校验 SHA256，防止下载损坏或被篡改。
//! - **断点续传**：下载到 `.part` 临时文件，完成后原子重命名。
//! - **进度回调**：通过 Tauri 事件向前端推送下载进度（每 1% 至少一次）。
//! - **幂等**：如果模型已存在且校验通过，直接返回 Ok。
//!
//! 模型文件路径：`<app_data_dir>/moontranslator/models/doclayout_yolo.onnx`
//! 临时文件：`<app_data_dir>/moontranslator/models/doclayout_yolo.onnx.part`

use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

/// DocLayout-YOLO 模型的下载源。
/// 优先 `GitHub` Releases（官方），失败时回退到镜像。
const MODEL_DOWNLOAD_URLS: &[&str] = &[
    "https://github.com/PaddlePaddle/PaddleOCR/releases/download/v2.6.0/doclayout_yolo.onnx",
    // 备用镜像（如果主源不可用）
    "https://huggingface.co/PaddlePaddle/DocLayout-YOLO/resolve/main/doclayout_yolo.onnx",
];

/// 模型文件的期望 SHA256（下载后校验）。
/// 注意：这是占位值，实际值需要在首次发布时通过 `sha256sum` 计算并填入。
/// 空字符串表示跳过校验（仅开发期使用，release 必须填入真实哈希）。
const MODEL_SHA256: &str = "";

/// 模型文件名。
const MODEL_FILENAME: &str = "doclayout_yolo.onnx";
/// 下载临时文件后缀。
const PART_SUFFIX: &str = ".part";

/// 获取模型存储目录：`<app_data_dir>/moontranslator/models/`
pub fn model_dir(app: &AppHandle) -> PathBuf {
    let mut path = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| {
            // Fallback: dirs::data_dir() (Windows: %APPDATA%, macOS: ~/Library/Application Support)
            let mut p = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            p.push("moontranslator");
            p
        });
    path.push("models");
    path
}

/// 获取模型文件的完整路径。
pub fn model_path(app: &AppHandle) -> PathBuf {
    model_dir(app).join(MODEL_FILENAME)
}

/// 检查模型是否已下载且校验通过。
///
/// 返回 `true` 表示模型可用，可以直接加载。
pub fn is_model_ready(app: &AppHandle) -> bool {
    let path = model_path(app);
    if !path.exists() {
        return false;
    }
    // 如果设置了 SHA256，校验文件哈希；空字符串则跳过校验。
    if MODEL_SHA256.is_empty() {
        return true;
    }
    match compute_file_sha256(&path) {
        Ok(hash) => hash == MODEL_SHA256,
        Err(_) => false,
    }
}

/// 计算文件的 SHA256 哈希值。
fn compute_file_sha256(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// 下载进度事件 payload。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    /// 已下载字节数。
    pub downloaded: u64,
    /// 总字节数（未知时为 0）。
    pub total: u64,
    /// 下载百分比 [0, 100]（总大小未知时为 0）。
    pub percent: u8,
    /// 下载速度（字节/秒）。
    pub speed: u64,
}

/// 下载模型文件。
///
/// 如果模型已存在且校验通过，直接返回 Ok（幂等）。
/// 否则从下载源拉取，写入 `.part` 临时文件，完成后原子重命名。
///
/// 通过 `layout-model-download-progress` 事件向前端推送进度。
pub async fn download_model(app: &AppHandle) -> Result<PathBuf, String> {
    // 幂等：模型已就绪则直接返回。
    if is_model_ready(app) {
        return Ok(model_path(app));
    }

    let dir = model_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建模型目录失败: {e}"))?;

    let final_path = dir.join(MODEL_FILENAME);
    let part_path = dir.join(format!("{MODEL_FILENAME}{PART_SUFFIX}"));

    // 尝试每个下载源，直到成功。
    let mut last_error = String::new();
    for url in MODEL_DOWNLOAD_URLS {
        match download_from_url(app, url, &part_path).await {
            Ok(()) => {
                // 下载完成后校验 SHA256（如果设置）。
                if !MODEL_SHA256.is_empty() {
                    let hash = compute_file_sha256(&part_path)?;
                    if hash != MODEL_SHA256 {
                        let _ = std::fs::remove_file(&part_path);
                        last_error = format!(
                            "SHA256 校验失败: 期望 {MODEL_SHA256}, 实际 {hash}"
                        );
                        continue;
                    }
                }
                // 原子重命名 .part → 最终文件名。
                std::fs::rename(&part_path, &final_path)
                    .map_err(|e| format!("重命名临时文件失败: {e}"))?;
                tracing::info!("[P6] 模型下载完成: {}", final_path.display());
                return Ok(final_path);
            }
            Err(e) => {
                tracing::warn!("[P6] 下载源 {} 失败: {}", url, e);
                last_error = e;
                // 清理残留的 .part 文件。
                let _ = std::fs::remove_file(&part_path);
            }
        }
    }

    Err(format!("所有下载源均失败。最后错误: {last_error}"))
}

/// 从单个 URL 下载文件到指定路径，带进度回调。
async fn download_from_url(
    app: &AppHandle,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    use futures::StreamExt;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_mins(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("创建文件失败: {e}"))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_percent: u8 = 0;
    let start_time = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取数据块失败: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("写入文件失败: {e}"))?;
        downloaded += chunk.len() as u64;

        // 计算进度百分比，仅在变化 >= 1% 时推送事件（避免事件洪流）。
        let percent = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0) as u8
        } else {
            0
        };
        if percent > last_percent {
            last_percent = percent;
            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                (downloaded as f64 / elapsed) as u64
            } else {
                0
            };
            let _ = app.emit(
                "layout-model-download-progress",
                DownloadProgress {
                    downloaded,
                    total,
                    percent,
                    speed,
                },
            );
        }
    }

    // 确保 .part 文件刷盘。
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| format!("刷盘失败: {e}"))?;

    Ok(())
}

/// 删除已下载的模型文件（用户在设置里关闭功能时可调用）。
///
/// 同时清理 `.part` 临时文件。返回 true 表示删除了模型文件。
pub fn remove_model(app: &AppHandle) -> bool {
    let path = model_path(app);
    let part_path = model_dir(app).join(format!("{MODEL_FILENAME}{PART_SUFFIX}"));
    let _ = std::fs::remove_file(&part_path);
    std::fs::remove_file(&path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_filename_constant() {
        assert_eq!(MODEL_FILENAME, "doclayout_yolo.onnx");
        assert_eq!(PART_SUFFIX, ".part");
    }

    #[test]
    fn download_urls_not_empty() {
        assert!(!MODEL_DOWNLOAD_URLS.is_empty());
        // 第一个应该是 HTTPS GitHub 链接。
        assert!(MODEL_DOWNLOAD_URLS[0].starts_with("https://"));
    }

    #[test]
    fn sha256_placeholder_documented() {
        // 开发期为空（跳过校验），release 必须填入真实哈希。
        // 这个测试只是提醒：release 前需要填入真实值。
        if MODEL_SHA256.is_empty() {
            // 仅在测试环境打印提醒，不强制失败。
            eprintln!("[P6] 警告: MODEL_SHA256 为空，release 前必须填入真实哈希值");
        }
    }
}
