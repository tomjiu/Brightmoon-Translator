//! Security validation utilities for input sanitization and injection prevention.
//!
//! This module provides centralized validation functions for:
//! - File path traversal prevention
//! - SQL LIKE pattern injection prevention
//! - Language code validation
//! - Plugin name sanitization
//! - Input length limits

use std::path::{Component, Path};
use std::sync::OnceLock;

/// Maximum allowed text length for translation requests (50,000 characters).
pub const MAX_TRANSLATION_TEXT_LENGTH: usize = 50_000;

/// Maximum allowed file path length.
pub const MAX_PATH_LENGTH: usize = 260; // Windows MAX_PATH

/// Maximum allowed plugin name length.
pub const MAX_PLUGIN_NAME_LENGTH: usize = 64;

/// Validate a file path to prevent path traversal attacks.
///
/// Checks:
/// - Path does not contain `..` components
/// - Path does not exceed MAX_PATH_LENGTH
/// - Path components are valid
///
/// Returns `Ok(())` if valid, `Err(message)` if invalid.
pub fn validate_file_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("File path is empty".to_string());
    }

    if path.len() > MAX_PATH_LENGTH {
        return Err(format!(
            "File path exceeds maximum length of {} characters",
            MAX_PATH_LENGTH
        ));
    }

    // Check for path traversal patterns
    let path_obj = Path::new(path);
    for component in path_obj.components() {
        match component {
            Component::ParentDir => {
                return Err("Path must not contain '..' components".to_string());
            },
            Component::RootDir if path.starts_with("//") || path.starts_with("\\\\") => {
                // UNC paths are allowed on Windows
            },
            _ => {},
        }
    }

    // Additional check: reject null bytes (can bypass checks on some systems)
    if path.contains('\0') {
        return Err("Path must not contain null bytes".to_string());
    }

    Ok(())
}

/// Validate a file path for write operations (output files).
///
/// In addition to `validate_file_path`, checks:
/// - Parent directory exists (if path has a parent)
pub fn validate_output_path(path: &str) -> Result<(), String> {
    validate_file_path(path)?;

    let out = Path::new(path);
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(format!(
                "Output directory does not exist: {}",
                parent.display()
            ));
        }
    }

    Ok(())
}

/// Escape special characters in a SQL LIKE pattern.
///
/// Escapes `%`, `_`, and `\` characters to prevent LIKE injection.
/// Use this before constructing LIKE patterns with user input.
///
/// Example:
/// ```
/// let safe = moontranslator_lib::security::sanitize_like_pattern("user%input_with_wildcards");
/// assert_eq!(safe, "user\\%input\\_with\\_wildcards");
/// ```
pub fn sanitize_like_pattern(pattern: &str) -> String {
    pattern
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Validate a language code (e.g., "en", "zh", "ja", "auto").
///
/// Accepts:
/// - ISO 639-1 two-letter codes (e.g., "en", "zh", "ja")
/// - ISO 639-1 with region (e.g., "zh-CN", "en-US")
/// - The special value "auto" for auto-detection
///
/// Returns `Ok(())` if valid, `Err(message)` if invalid.
pub fn validate_language_code(code: &str) -> Result<(), String> {
    if code.is_empty() {
        return Err("Language code is empty".to_string());
    }

    if code.len() > 10 {
        return Err("Language code is too long".to_string());
    }

    // Allow "auto" as a special case
    if code == "auto" {
        return Ok(());
    }

    // Must be alphanumeric with optional hyphens
    if !code
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("Invalid language code: {}", code));
    }

    Ok(())
}

/// Sanitize a plugin name to prevent path traversal and injection.
///
/// Only allows alphanumeric characters, hyphens, underscores, and dots.
/// Rejects names containing path separators or traversal patterns.
pub fn sanitize_plugin_name(name: &str) -> Result<String, String> {
    if name.is_empty() {
        return Err("Plugin name is empty".to_string());
    }

    if name.len() > MAX_PLUGIN_NAME_LENGTH {
        return Err(format!(
            "Plugin name exceeds maximum length of {} characters",
            MAX_PLUGIN_NAME_LENGTH
        ));
    }

    // Reject path traversal patterns
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("Plugin name must not contain path separators or '..'".to_string());
    }

    // Reject null bytes
    if name.contains('\0') {
        return Err("Plugin name must not contain null bytes".to_string());
    }

    // Only allow safe characters
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    Ok(sanitized)
}

/// Validate text input length for translation.
///
/// Returns `Ok(())` if within limits, `Err(message)` if too long.
pub fn validate_text_length(text: &str, max_length: usize) -> Result<(), String> {
    if text.len() > max_length {
        return Err(format!(
            "Text exceeds maximum length of {} characters (got {})",
            max_length,
            text.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_file_path_normal() {
        assert!(validate_file_path("C:\\Users\\test\\file.txt").is_ok());
        assert!(validate_file_path("/home/user/file.txt").is_ok());
        assert!(validate_file_path("relative/path/file.txt").is_ok());
    }

    #[test]
    fn test_validate_file_path_traversal() {
        assert!(validate_file_path("../etc/passwd").is_err());
        assert!(validate_file_path("C:\\Users\\..\\Windows\\system32").is_err());
        assert!(validate_file_path("path/../../../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_file_path_null_byte() {
        assert!(validate_file_path("path/file\0.txt").is_err());
    }

    #[test]
    fn test_validate_file_path_empty() {
        assert!(validate_file_path("").is_err());
    }

    #[test]
    fn test_sanitize_like_pattern() {
        assert_eq!(sanitize_like_pattern("hello"), "hello");
        assert_eq!(sanitize_like_pattern("100%"), "100\\%");
        assert_eq!(sanitize_like_pattern("user_name"), "user\\_name");
        assert_eq!(sanitize_like_pattern("path\\to"), "path\\\\to");
        assert_eq!(sanitize_like_pattern("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn test_validate_language_code() {
        assert!(validate_language_code("en").is_ok());
        assert!(validate_language_code("zh-CN").is_ok());
        assert!(validate_language_code("auto").is_ok());
        assert!(validate_language_code("").is_err());
        assert!(validate_language_code("verylongcode123").is_err());
    }

    #[test]
    fn test_sanitize_plugin_name() {
        assert_eq!(sanitize_plugin_name("my-plugin").unwrap(), "my-plugin");
        assert_eq!(
            sanitize_plugin_name("valid_name_123").unwrap(),
            "valid_name_123"
        );
        assert!(sanitize_plugin_name("../etc/passwd").is_err());
        assert!(sanitize_plugin_name("path/to/plugin").is_err());
        assert!(sanitize_plugin_name("").is_err());
    }

    #[test]
    fn test_validate_text_length() {
        assert!(validate_text_length("short text", 100).is_ok());
        assert!(validate_text_length(&"a".repeat(101), 100).is_err());
    }
}

// ===========================================================================
// Sensitive Data Protection
// ===========================================================================

// ---------------------------------------------------------------------------
// SecureString: zeroizes memory on drop
// ---------------------------------------------------------------------------

/// A string wrapper that zeroizes its contents on drop.
/// Use this for any in-memory handling of API keys / secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecureString(String);

impl SecureString {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string (caller must zeroize manually).
    pub fn into_inner(self) -> String {
        let s = self.0.clone();
        std::mem::forget(self);
        s
    }

    /// Get a masked version for display: show first 4 and last 4 chars.
    pub fn masked(&self) -> String {
        mask_api_key(&self.0)
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        // Zero out the underlying string bytes.
        // SAFETY: String::as_mut_vec is sound during Drop because self owns
        // the buffer for the remainder of drop; bytes remain valid until then.
        unsafe {
            let bytes = self.0.as_mut_vec();
            zeroize::Zeroize::zeroize(bytes);
        }
    }
}

impl AsRef<str> for SecureString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.masked())
    }
}

// ---------------------------------------------------------------------------
// DPAPI encryption (Windows only)
// ---------------------------------------------------------------------------

/// Encrypt plaintext using Windows DPAPI (Current User scope).
/// Returns the ciphertext bytes, or an error string.
#[cfg(target_os = "windows")]
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };

    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    // SAFETY: input_blob points to `plaintext` (valid for the call duration)
    // and output_blob is a stack-allocated CRYPT_INTEGER_BLOB. CryptProtectData
    // fills output_blob.pbData with an allocated buffer the caller must free.
    let success = unsafe {
        CryptProtectData(
            &mut input_blob,
            windows::core::PCWSTR::null(),
            None,
            None,
            None,
            0,
            &mut output_blob,
        )
    };

    if success.is_ok() {
        // SAFETY: CryptProtectData populated output_blob.pbData with a buffer
        // of cbData bytes; both fields are set by the API on success.
        let encrypted = unsafe {
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
        };
        // SAFETY: LocalFree releases the DPAPI-allocated output buffer exactly
        // once after it has been copied into `encrypted`.
        unsafe {
            let _ = LocalFree(HLOCAL(output_blob.pbData as *mut core::ffi::c_void));
        }
        Ok(encrypted)
    } else {
        Err("DPAPI CryptProtectData failed".to_string())
    }
}

/// Decrypt ciphertext using Windows DPAPI (Current User scope).
#[cfg(target_os = "windows")]
pub fn dpapi_decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let mut input_blob = CRYPT_INTEGER_BLOB {
        cbData: ciphertext.len() as u32,
        pbData: ciphertext.as_ptr() as *mut u8,
    };

    let mut output_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let mut desc_ptr = windows::core::PWSTR::null();

    // SAFETY: input_blob points to `ciphertext` (valid for the call duration)
    // and output_blob/desc_ptr are stack-allocated. CryptUnprotectData fills
    // output_blob.pbData and (optionally) desc_ptr with caller-freed buffers.
    let success = unsafe {
        CryptUnprotectData(
            &mut input_blob,
            Some(&mut desc_ptr),
            None,
            None,
            None,
            0,
            &mut output_blob,
        )
    };

    if success.is_ok() {
        // SAFETY: CryptUnprotectData populated output_blob.pbData with a buffer
        // of cbData bytes; both fields are set by the API on success.
        let decrypted = unsafe {
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
        };
        // SAFETY: LocalFree releases the DPAPI-allocated buffers exactly once
        // after they have been copied into `decrypted`. desc_ptr is null-checked.
        unsafe {
            let _ = LocalFree(HLOCAL(output_blob.pbData as *mut core::ffi::c_void));
            if !desc_ptr.is_null() {
                let _ = LocalFree(HLOCAL(desc_ptr.as_ptr() as *mut core::ffi::c_void));
            }
        }
        Ok(decrypted)
    } else {
        Err("DPAPI CryptUnprotectData failed".to_string())
    }
}

// Non-Windows stub: fall back to AES-GCM only (functional but not OS-level protection)
#[cfg(not(target_os = "windows"))]
pub fn dpapi_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    aes_encrypt(plaintext)
}

#[cfg(not(target_os = "windows"))]
pub fn dpapi_decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    decrypt_aes(ciphertext)
}

// ---------------------------------------------------------------------------
// AES-256-GCM encryption (cross-platform, key derived from machine ID)
// ---------------------------------------------------------------------------

/// Derive a 256-bit key from a machine-specific seed.
fn derive_machine_key() -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    // Mix in hostname
    if let Ok(hostname) = std::env::var("COMPUTERNAME") {
        hasher.update(hostname.as_bytes());
    } else if let Ok(hostname) = std::env::var("HOSTNAME") {
        hasher.update(hostname.as_bytes());
    }

    // Mix in username
    if let Ok(username) = std::env::var("USERNAME") {
        hasher.update(username.as_bytes());
    } else if let Ok(username) = std::env::var("USER") {
        hasher.update(username.as_bytes());
    }

    // Mix in app-specific salt
    hasher.update(b"moontranslator-security-v1");

    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

/// Get or compute the machine key (cached).
fn machine_key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(derive_machine_key)
}

/// Encrypt plaintext using AES-256-GCM with a machine-derived key.
/// Returns nonce (12 bytes) + ciphertext + tag (16 bytes).
pub fn aes_encrypt(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
    use rand::RngCore;

    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(machine_key());
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("AES-GCM encrypt error: {}", e))?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

/// Decrypt ciphertext produced by `aes_encrypt`.
pub fn decrypt_aes(data: &[u8]) -> Result<Vec<u8>, String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};

    if data.len() < 12 + 16 {
        return Err("Invalid encrypted data: too short".to_string());
    }

    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(machine_key());
    let cipher = Aes256Gcm::new(key);

    let (nonce_bytes, ciphertext) = data.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("AES-GCM decrypt error: {}", e))
}

// ---------------------------------------------------------------------------
// Encrypted config value encoding helpers
// ---------------------------------------------------------------------------

/// The magic prefix that marks a value as DPAPI-encrypted in config.json.
const DPAPI_MAGIC: &str = "DPAPI:";

/// The magic prefix that marks a value as AES-GCM encrypted in config.json.
const AES_MAGIC: &str = "AES:";

/// Encrypt a secret string for storage in config.json.
/// Uses DPAPI on Windows, AES-GCM as fallback / cross-platform.
pub fn encrypt_secret(plaintext: &str) -> String {
    if plaintext.is_empty() {
        return String::new();
    }

    // Try DPAPI first on Windows
    #[cfg(target_os = "windows")]
    {
        if let Ok(encrypted) = dpapi_encrypt(plaintext.as_bytes()) {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&encrypted);
            // Zeroize the raw ciphertext bytes
            let mut enc = encrypted;
            zeroize::Zeroize::zeroize(&mut enc);
            return format!("{}{}", DPAPI_MAGIC, encoded);
        }
        tracing::warn!("DPAPI encryption failed, falling back to AES-GCM");
    }

    // AES-GCM fallback
    if let Ok(encrypted) = aes_encrypt(plaintext.as_bytes()) {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&encrypted);
        format!("{}{}", AES_MAGIC, encoded)
    } else {
        tracing::error!("All encryption methods failed, storing plaintext (INSECURE)");
        plaintext.to_string()
    }
}

/// Decrypt a secret string from config.json.
/// Detects the encryption method by magic prefix.
pub fn decrypt_secret(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }

    if let Some(encoded) = stored.strip_prefix(DPAPI_MAGIC) {
        use base64::Engine;
        let ciphertext = match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(c) => c,
            Err(_) => return stored.to_string(),
        };
        match dpapi_decrypt(&ciphertext) {
            Ok(decrypted) => {
                let result = String::from_utf8_lossy(&decrypted).to_string();
                // Zeroize the raw decrypted bytes
                let mut dec = decrypted;
                zeroize::Zeroize::zeroize(&mut dec);
                result
            },
            Err(_) => stored.to_string(),
        }
    } else if let Some(encoded) = stored.strip_prefix(AES_MAGIC) {
        use base64::Engine;
        let ciphertext = match base64::engine::general_purpose::STANDARD.decode(encoded) {
            Ok(c) => c,
            Err(_) => return stored.to_string(),
        };
        match decrypt_aes(&ciphertext) {
            Ok(decrypted) => String::from_utf8_lossy(&decrypted).to_string(),
            Err(_) => stored.to_string(),
        }
    } else {
        // No encryption prefix -- plaintext (legacy or not yet encrypted)
        stored.to_string()
    }
}

/// Mask an API key for display: show first 4 and last 4 chars.
pub fn mask_api_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = key.chars().collect();
    let len = chars.len();
    if len <= 8 {
        "*".repeat(len)
    } else {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[len - 4..].iter().collect();
        format!("{}***{}", prefix, suffix)
    }
}

// ---------------------------------------------------------------------------
// Secure file deletion: overwrite before delete
// ---------------------------------------------------------------------------

/// Securely delete a file by overwriting its contents with random data
/// before removing it. Prevents basic data recovery.
pub fn secure_delete(path: &Path) -> std::io::Result<()> {
    use rand::RngCore;

    if !path.exists() {
        return Ok(());
    }

    let metadata = std::fs::metadata(path)?;
    let file_len = metadata.len() as usize;

    if file_len > 0 {
        let mut rng = rand::thread_rng();

        // Pass 1: random data
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
            use std::io::Write;
            let chunk_size = file_len.min(65536);
            let mut remaining = file_len;
            while remaining > 0 {
                let size = remaining.min(chunk_size);
                let mut buf = vec![0u8; size];
                rng.fill_bytes(&mut buf);
                let _ = file.write_all(&buf);
                zeroize::Zeroize::zeroize(&mut buf);
                remaining -= size;
            }
            let _ = file.flush();
        }

        // Pass 2: zeros
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
            use std::io::Write;
            let zeros = vec![0u8; file_len.min(65536)];
            let mut remaining = file_len;
            while remaining > 0 {
                let size = remaining.min(zeros.len());
                let _ = file.write_all(&zeros[..size]);
                remaining -= size;
            }
            let _ = file.flush();
        }

        // Pass 3: random data again
        if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
            use std::io::Write;
            let chunk_size = file_len.min(65536);
            let mut remaining = file_len;
            while remaining > 0 {
                let size = remaining.min(chunk_size);
                let mut buf = vec![0u8; size];
                rng.fill_bytes(&mut buf);
                let _ = file.write_all(&buf);
                zeroize::Zeroize::zeroize(&mut buf);
                remaining -= size;
            }
            let _ = file.flush();
        }
    }

    std::fs::remove_file(path)
}

// ---------------------------------------------------------------------------
// Log sanitization helpers
// ---------------------------------------------------------------------------

/// Sanitize a log message string, replacing sensitive patterns with `***`.
/// Apply this to error messages before logging to prevent API key leakage.
pub fn sanitize_log_message(message: &str) -> String {
    let mut result = message.to_string();

    // 1. sk-... (OpenAI/DeepSeek-style keys)
    result = sanitize_pattern_regex(&result, r"sk-[A-Za-z0-9]{20,}", "sk-***");
    // 2. Bearer <token>
    result = sanitize_pattern_regex(&result, r"Bearer [A-Za-z0-9_\-\.]{20,}", "Bearer ***");
    // 3. api_key=... or api_key: ...
    result = sanitize_pattern_regex(
        &result,
        r"(?i)api_key[=:]\s*[A-Za-z0-9_\-]{10,}",
        "api_key=***",
    );
    // 4. secret=... or secret: ...
    result = sanitize_pattern_regex(
        &result,
        r"(?i)secret[=:]\s*[A-Za-z0-9_\-]{10,}",
        "secret=***",
    );
    // 5. DeepL-Auth-Key <key>
    result = sanitize_pattern_regex(
        &result,
        r"DeepL-Auth-Key [A-Za-z0-9_\-]{10,}",
        "DeepL-Auth-Key ***",
    );
    // 6. Authorization header with long token
    result = sanitize_pattern_regex(
        &result,
        r"(?i)authorization[=:]\s*[A-Za-z0-9_\-\.]{20,}",
        "Authorization=***",
    );
    // 7. appid/sign/sign patterns from Baidu API
    result = sanitize_pattern_regex(
        &result,
        r"(?i)(appid|sign)[=:]\s*[A-Za-z0-9]{20,}",
        "$1=***",
    );

    result
}

/// Helper to apply a single regex replacement.
fn sanitize_pattern_regex(text: &str, pattern: &str, replacement: &str) -> String {
    match regex::Regex::new(pattern) {
        Ok(re) => re.replace_all(text, replacement).to_string(),
        Err(_) => text.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Additional tests for sensitive data protection
// ---------------------------------------------------------------------------

#[cfg(test)]
mod protection_tests {
    use super::*;

    #[test]
    fn test_secure_string_masked() {
        let s = SecureString::new("1234567890abcdef".to_string());
        assert_eq!(s.masked(), "1234***cdef");
    }

    #[test]
    fn test_secure_string_masked_short() {
        let s = SecureString::new("short".to_string());
        assert_eq!(s.masked(), "*****");
    }

    #[test]
    fn test_mask_api_key() {
        assert_eq!(mask_api_key("1234567890abcdef"), "1234***cdef");
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("short"), "*****");
    }

    #[test]
    fn test_aes_encrypt_decrypt_roundtrip() {
        let plaintext = b"test-api-key-12345";
        let encrypted = aes_encrypt(plaintext).unwrap();
        let decrypted = decrypt_aes(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_aes_decrypt_tampered_data() {
        let plaintext = b"test-api-key-12345";
        let mut encrypted = aes_encrypt(plaintext).unwrap();
        // Tamper with the ciphertext
        let len = encrypted.len();
        encrypted[len - 1] ^= 0xFF;
        assert!(decrypt_aes(&encrypted).is_err());
    }

    #[test]
    fn test_encrypt_decrypt_secret_roundtrip() {
        let secret = "my-super-secret-api-key-12345";
        let encrypted = encrypt_secret(secret);
        // Should have a magic prefix
        assert!(encrypted.starts_with(DPAPI_MAGIC) || encrypted.starts_with(AES_MAGIC));
        let decrypted = decrypt_secret(&encrypted);
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_decrypt_secret_plaintext_fallback() {
        // Legacy plaintext values should be returned as-is
        let plaintext = "plain-text-value";
        assert_eq!(decrypt_secret(plaintext), plaintext);
    }

    #[test]
    fn test_encrypt_empty_string() {
        assert_eq!(encrypt_secret(""), "");
    }

    #[test]
    fn test_decrypt_empty_string() {
        assert_eq!(decrypt_secret(""), "");
    }

    #[test]
    fn test_sanitize_sk_key() {
        let msg = "Error with key sk-abc123456789012345678901234567890 in request";
        let sanitized = sanitize_log_message(msg);
        assert!(!sanitized.contains("sk-abc1234567890"));
        assert!(sanitized.contains("sk-***"));
    }

    #[test]
    fn test_sanitize_bearer_token() {
        let msg = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0";
        let sanitized = sanitize_log_message(msg);
        assert!(!sanitized.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(sanitized.contains("Bearer ***"));
    }

    #[test]
    fn test_sanitize_api_key_param() {
        let msg = "Request: api_key=sk-1234567890abcdef1234567890";
        let sanitized = sanitize_log_message(msg);
        assert!(!sanitized.contains("1234567890abcdef"));
    }

    #[test]
    fn test_sanitize_deepl_key() {
        let msg = "Using DeepL-Auth-Key abc123-def456-ghi789-jkl012";
        let sanitized = sanitize_log_message(msg);
        assert!(sanitized.contains("DeepL-Auth-Key ***"));
    }
}
