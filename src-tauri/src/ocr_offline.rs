//! Offline OCR via external RapidOCR / PaddleOCR-json sidecars (pot-style).
//! Models/exe are NOT bundled; user sets plugin_dir.

use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

/// RAII guard that deletes a temp file on drop.
/// S2-3: ensures cleanup even on early `?` return or panic, so the sidecar
/// temp PNG does not leak when the process is killed mid-OCR.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Run offline OCR on PNG bytes.
/// `backend`: "rapid" | "paddle"
/// `plugin_dir`: directory containing exe + models (user-provided).
pub fn run_offline_ocr(
    png_bytes: &[u8],
    backend: &str,
    plugin_dir: &str,
    lang: Option<&str>,
) -> Result<String, String> {
    let dir = PathBuf::from(plugin_dir.trim());
    if plugin_dir.trim().is_empty() || !dir.is_dir() {
        return Err(
            "Offline OCR plugin_dir is empty or missing. Set path to Rapid/Paddle extract folder."
                .into(),
        );
    }

    let id = Uuid::new_v4().to_string();
    let temp_path = std::env::temp_dir().join(format!("moontranslator_ocr_{id}.png"));
    std::fs::write(&temp_path, png_bytes).map_err(|e| format!("OCR temp write failed: {e}"))?;
    // S2-3: guard the temp file so it is deleted even if run_rapid/run_paddle
    // returns Err early or panics. Previously the cleanup at the end of the
    // function was skipped on early `?` paths, leaking moontranslator_ocr_*.png.
    let _guard = TempFileGuard(temp_path.clone());

    match backend.trim().to_ascii_lowercase().as_str() {
        "paddle" => run_paddle(&dir, &temp_path, lang),
        _ => run_rapid(&dir, &temp_path, lang),
    }
}

fn run_rapid(plugin_dir: &Path, image: &Path, lang: Option<&str>) -> Result<String, String> {
    let exe = if cfg!(windows) {
        plugin_dir.join("RapidOcrOnnx.exe")
    } else {
        plugin_dir.join("RapidOcrOnnx")
    };
    if !exe.exists() {
        // also try nested x86_64-pc-windows-msvc layout from pot zip
        let alt = plugin_dir
            .join("x86_64-pc-windows-msvc")
            .join("RapidOcrOnnx.exe");
        if alt.exists() {
            return run_rapid_exe(&alt, plugin_dir, image, lang);
        }
        return Err(format!(
            "RapidOcrOnnx not found under {}. Download pot-app-recognize-plugin-rapid and set plugin_dir.",
            plugin_dir.display()
        ));
    }
    run_rapid_exe(&exe, plugin_dir, image, lang)
}

fn run_rapid_exe(
    exe: &Path,
    plugin_dir: &Path,
    image: &Path,
    lang: Option<&str>,
) -> Result<String, String> {
    let lang_key = match lang.unwrap_or("ch") {
        "en" | "eng" => "en",
        "ja" | "jp" => "japan",
        "ko" => "korean",
        _ => "ch",
    };
    // pot plugin uses models relative to plugin dir
    let models = plugin_dir.join("models");
    let models_arg = if models.is_dir() {
        models.to_string_lossy().to_string()
    } else {
        "models".into()
    };

    let output = Command::new(exe)
        .current_dir(plugin_dir)
        .args([
            "--models",
            &models_arg,
            "--det",
            "ch_PP-OCR_det_infer.onnx",
            "--cls",
            "ch_ppocr_mobile_v2.0_cls_infer.onnx",
            "--rec",
            &format!("{lang_key}_PP-OCR_rec_infer.onnx"),
            "--keys",
            &format!("{lang_key}_dict.txt"),
            "--image",
            &image.to_string_lossy(),
            "--numThread",
            "4",
            "--padding",
            "50",
            "--maxSideLen",
            "1024",
            "--boxScoreThresh",
            "0.5",
            "--boxThresh",
            "0.5",
            "--unClipRatio",
            "1.6",
            "--doAngle",
            "1",
            "--mostAngle",
            "1",
        ])
        .output()
        .map_err(|e| format!("Failed to spawn RapidOCR: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("RapidOCR failed: {err}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_rapid_stdout(&stdout)
}

fn parse_rapid_stdout(stdout: &str) -> Result<String, String> {
    // pot: split on =====End detect===== then after "s)"
    if let Some(rest) = stdout.split("=====End detect=====").nth(1) {
        if let Some(idx) = rest.find("s)") {
            let text = rest[idx + 2..].trim();
            if !text.is_empty() {
                return Ok(text.to_string());
            }
        }
        let t = rest.trim();
        if !t.is_empty() {
            return Ok(t.to_string());
        }
    }
    let t = stdout.trim();
    if t.is_empty() {
        Err("RapidOCR returned empty output".into())
    } else {
        Ok(t.to_string())
    }
}

fn run_paddle(plugin_dir: &Path, image: &Path, lang: Option<&str>) -> Result<String, String> {
    let exe = plugin_dir.join("PaddleOCR-json.exe");
    if !exe.exists() {
        return Err(format!(
            "PaddleOCR-json.exe not found under {}. Place PaddleOCR-json release there.",
            plugin_dir.display()
        ));
    }
    let lang_key = match lang.unwrap_or("ch") {
        "en" | "eng" => "en",
        _ => "ch",
    };
    let config = format!("models/config_{lang_key}.txt");
    let output = Command::new(&exe)
        .current_dir(plugin_dir)
        .args([
            "use_angle_cls=true",
            "cls=true",
            &format!("--image_path={}", image.to_string_lossy()),
            &format!("--config_path={config}"),
        ])
        .output()
        .map_err(|e| format!("Failed to spawn PaddleOCR: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PaddleOCR failed: {err}"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // pot: after "OCR init completed." then JSON
    let json_part = stdout
        .split("OCR init completed.")
        .nth(1)
        .unwrap_or(&stdout)
        .trim();
    let v: serde_json::Value = serde_json::from_str(json_part)
        .map_err(|e| format!("PaddleOCR JSON parse: {e} — {json_part}"))?;
    let mut text = String::new();
    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for line in arr {
            if let Some(t) = line.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
        }
    }
    if text.trim().is_empty() {
        Err("PaddleOCR: no text".into())
    } else {
        Ok(text.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rapid_sample() {
        let s = "foo\n=====End detect=====\n0.12s)\nHello\nWorld\n";
        let t = parse_rapid_stdout(s).unwrap();
        assert!(t.contains("Hello"));
    }
}
