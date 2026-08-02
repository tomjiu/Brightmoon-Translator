//! 4d: Luna Hook H-Code string parser.
//!
//! Parses H-Code strings (e.g. `/HW-4@12345:game.exe`) into a structured
//! form that the host can use to:
//! 1. Resolve the target module in the remote process
//! 2. Compute the absolute hook address (module_base + RVA)
//! 3. Call `HookInstallAtAddress` via CreateRemoteThread
//!
//! ## Supported format (simplified subset of Luna Hook spec)
//!
//! ```text
//! /H{type}[-{neg_offset}]{data_offset}[:{deref_offset}[:{split_offset}[:{split_index}]]]@{addr}:{module}
//! ```
//!
//! ### Hook type letters (case-insensitive)
//! - `A` / `S`: ANSI string (single-byte, code page from config)
//! - `W`: Wide string (UTF-16, code_page=0 in our protocol)
//! - `N`: Null-terminated (default; same as A for our purposes)
//! - `R`: Reversed text (we don't reverse; just flag it)
//! - Other letters: accepted but treated as ANSI
//!
//! ### Examples
//! - `/HA-4@12345:game.exe` — ANSI, data_offset=-4, RVA 0x12345 in game.exe
//! - `/HW-4@12345:game.exe` — UTF-16, data_offset=-4, RVA 0x12345
//! - `/HS-20@12345:game.exe` — ANSI, data_offset=-0x20, RVA 0x12345
//! - `/HA10:4@12345:game.exe` — ANSI, data_offset=0x10, deref_offset=4
//!
//! ## Non-goals (deferred)
//! - T-Code (text replacement templates)
//! - Full Luna Hook semantics (R=reverse, Q=quick pause, etc.)
//! - Split hooks (multi-pointer extraction) — parsed but not wired to DLL

use serde::{Deserialize, Serialize};

/// Hook type extracted from the H-Code letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HookTextType {
    /// Single-byte ANSI string; code_page from config (932 for ja, 936 for zh-CN, etc.)
    Ansi,
    /// UTF-16 wide string; code_page=0 in our DLL protocol.
    Wide,
    /// Null-terminated (same encoding as Ansi but explicit); treated as Ansi.
    NullTerminated,
    /// Reversed text flag (we don't reverse; callers can post-process).
    Reversed,
    /// Unknown type letter — preserved for forward compat, treated as Ansi.
    Other(char),
}

impl HookTextType {
    /// Map hook type to the code_page value expected by `HookInstallAtAddress`.
    /// 0 = UTF-16 (wide), any other = ANSI code page.
    pub fn code_page(&self, default_ansi_cp: u32) -> u32 {
        match self {
            HookTextType::Wide => 0,
            HookTextType::Ansi
            | HookTextType::NullTerminated
            | HookTextType::Reversed
            | HookTextType::Other(_) => default_ansi_cp,
        }
    }

    /// Whether text should be byte-reversed before sending (R type).
    pub fn is_reversed(&self) -> bool {
        matches!(self, HookTextType::Reversed)
    }
}

/// Parsed H-Code parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookCode {
    /// Hook type letter (A/W/N/R/...).
    pub text_type: HookTextType,
    /// Byte offset from the hook address where the text pointer (or raw bytes) live.
    /// Negative = before hook address (common for stack-relative reads).
    pub data_offset: i32,
    /// Optional dereference offset. When present, the value at `data_offset`
    /// is treated as a pointer, and we dereference `deref_levels` times.
    /// (Luna Hook's `:N` syntax — we simplify to "1 level if present".)
    pub deref_offset: Option<i32>,
    /// Effective dereference level (0 = read bytes directly, 1 = single deref).
    pub deref_levels: u32,
    /// Optional split offset (parsed but not yet wired to DLL).
    pub split_offset: Option<i32>,
    /// Optional split index (parsed but not yet wired to DLL).
    pub split_index: Option<u32>,
    /// Hex address. If `< 0x10000`, treated as RVA into `module`.
    /// Otherwise treated as absolute virtual address (module ignored).
    pub addr: u64,
    /// Module name (e.g. "game.exe"). Case-insensitive match against
    /// loaded modules in the target process.
    pub module: String,
}

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookCodeError {
    /// String doesn't start with `/H`.
    MissingPrefix,
    /// Missing `@addr:module` suffix.
    MissingAtModule,
    /// Hook type letter missing or invalid.
    MissingType,
    /// Address is not valid hex.
    InvalidAddress,
    /// Module name empty.
    EmptyModule,
    /// Data offset is not valid hex.
    InvalidOffset(String),
}

impl std::fmt::Display for HookCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookCodeError::MissingPrefix => write!(f, "H-Code must start with /H"),
            HookCodeError::MissingAtModule => write!(f, "H-Code missing @addr:module suffix"),
            HookCodeError::MissingType => write!(f, "H-Code missing type letter after /H"),
            HookCodeError::InvalidAddress => write!(f, "H-Code address is not valid hex"),
            HookCodeError::EmptyModule => write!(f, "H-Code module name is empty"),
            HookCodeError::InvalidOffset(s) => write!(f, "H-Code offset '{}' is not valid hex", s),
        }
    }
}

impl std::error::Error for HookCodeError {}

/// Parse a Luna Hook H-Code string.
///
/// Accepted forms (case-insensitive type letter):
/// - `/H{type}@{addr}:{module}`
/// - `/H{type}{offset}@{addr}:{module}`
/// - `/H{type}-{neg}@{addr}:{module}`
/// - `/H{type}{offset}:{deref}@{addr}:{module}`
/// - `/H{type}{offset}:{deref}:{split}:{split_idx}@{addr}:{module}`
///
/// Offsets are hex (with optional `-` prefix for negative).
/// Address is hex. Module is the bare filename (no path).
pub fn parse_h_code(input: &str) -> Result<HookCode, HookCodeError> {
    let s = input.trim();

    // Strip optional leading /H or H
    let after_h = if let Some(rest) = s.strip_prefix("/H") {
        rest
    } else if let Some(rest) = s.strip_prefix("H") {
        rest
    } else {
        return Err(HookCodeError::MissingPrefix);
    };

    if after_h.is_empty() {
        return Err(HookCodeError::MissingType);
    }

    // Split at '@' — everything before is the "params" part, after is "addr:module".
    let at_pos = after_h.find('@').ok_or(HookCodeError::MissingAtModule)?;
    let params = &after_h[..at_pos];
    let addr_module = &after_h[at_pos + 1..];

    // Parse type letter (first char of params).
    let type_char = params.chars().next().unwrap();
    let text_type = match type_char.to_ascii_uppercase() {
        'A' | 'S' => HookTextType::Ansi,
        'W' => HookTextType::Wide,
        'N' => HookTextType::NullTerminated,
        'R' => HookTextType::Reversed,
        other => HookTextType::Other(other),
    };

    // Remaining params after type letter: optional offsets.
    // Format: [-{neg}]{data_offset}[:{deref}][:{split}[:{split_idx}]]
    let rest = &params[type_char.len_utf8()..];

    // Split on ':' to get up to 4 parts: data_offset, deref, split, split_idx
    let parts: Vec<&str> = rest.splitn(4, ':').collect();
    let data_offset_str = parts.first().copied().unwrap_or("");
    let deref_str = parts.get(1).copied();
    let split_str = parts.get(2).copied();
    let split_idx_str = parts.get(3).copied();

    let data_offset = parse_hex_offset(data_offset_str)
        .ok_or_else(|| HookCodeError::InvalidOffset(data_offset_str.to_string()))?;

    let (deref_offset, deref_levels) = if let Some(ds) = deref_str {
        if ds.is_empty() {
            (None, 0)
        } else {
            let d = parse_hex_offset(ds)
                .ok_or_else(|| HookCodeError::InvalidOffset(ds.to_string()))?;
            (Some(d), 1)
        }
    } else {
        (None, 0)
    };

    let split_offset = split_str
        .filter(|s| !s.is_empty())
        .and_then(|s| parse_hex_offset(s));

    let split_index = split_idx_str
        .filter(|s| !s.is_empty())
        .and_then(|s| u32::from_str_radix(s, 16).ok());

    // Parse addr:module. Module may contain colons (drive letters), so split
    // on the FIRST colon only.
    let colon_pos = addr_module
        .find(':')
        .ok_or(HookCodeError::MissingAtModule)?;
    let addr_str = &addr_module[..colon_pos];
    let module = addr_module[colon_pos + 1..].trim().to_string();

    if module.is_empty() {
        return Err(HookCodeError::EmptyModule);
    }

    let addr = u64::from_str_radix(addr_str.trim(), 16)
        .map_err(|_| HookCodeError::InvalidAddress)?;

    Ok(HookCode {
        text_type,
        data_offset,
        deref_offset,
        deref_levels,
        split_offset,
        split_index,
        addr,
        module,
    })
}

/// Parse a hex offset string that may have a leading `-` for negative values.
/// Returns None on parse failure.
fn parse_hex_offset(s: &str) -> Option<i32> {
    let s = s.trim();
    if s.is_empty() {
        // Empty offset = 0 (common in `/HW@addr:module` with no explicit offset)
        return Some(0);
    }
    let (neg, digits) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = s.strip_prefix('+') {
        (false, rest)
    } else {
        (false, s)
    };
    // Strip optional 0x prefix
    let digits = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")).unwrap_or(digits);
    if digits.is_empty() {
        return Some(0);
    }
    let val = i64::from_str_radix(digits, 16).ok()?;
    let signed = if neg { -val } else { val };
    // Range-check to i32
    signed.try_into().ok()
}

/// Whether `addr` should be treated as an RVA (relative to module base)
/// rather than an absolute virtual address.
///
/// Luna Hook convention: addresses < 0x10000 are RVAs. Larger addresses
/// are absolute. The host uses this to decide whether to add module_base.
pub fn is_rva(addr: u64) -> bool {
    addr < 0x10000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_ansi() {
        let hc = parse_h_code("/HA-4@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Ansi);
        assert_eq!(hc.data_offset, -4);
        assert_eq!(hc.deref_levels, 0);
        assert_eq!(hc.addr, 0x12345);
        assert_eq!(hc.module, "game.exe");
        assert!(!is_rva(hc.addr)); // 0x12345 > 0x10000, absolute address
    }

    #[test]
    fn parse_wide_string() {
        let hc = parse_h_code("/HW-4@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Wide);
        assert_eq!(hc.data_offset, -4);
        assert_eq!(hc.text_type.code_page(932), 0); // wide = code_page 0
    }

    #[test]
    fn parse_null_terminated() {
        let hc = parse_h_code("/HN-4@DEADBEEF:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::NullTerminated);
        assert_eq!(hc.addr, 0xDEADBEEF);
        assert!(!is_rva(hc.addr)); // absolute address
    }

    #[test]
    fn parse_with_deref() {
        let hc = parse_h_code("/HA10:4@12345:game.exe").unwrap();
        assert_eq!(hc.data_offset, 0x10);
        assert_eq!(hc.deref_offset, Some(4));
        assert_eq!(hc.deref_levels, 1);
    }

    #[test]
    fn parse_with_split() {
        let hc = parse_h_code("/HA10:4:8:1@12345:game.exe").unwrap();
        assert_eq!(hc.data_offset, 0x10);
        assert_eq!(hc.deref_offset, Some(4));
        assert_eq!(hc.split_offset, Some(8));
        assert_eq!(hc.split_index, Some(1));
    }

    #[test]
    fn parse_no_offset() {
        // /HW@addr:module — no explicit offset (defaults to 0)
        let hc = parse_h_code("/HW@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Wide);
        assert_eq!(hc.data_offset, 0);
    }

    #[test]
    fn parse_reversed() {
        let hc = parse_h_code("/HR-4@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Reversed);
        assert!(hc.text_type.is_reversed());
    }

    #[test]
    fn parse_unknown_type_passes_through() {
        let hc = parse_h_code("/HX-4@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Other('X'));
        // Unknown types default to ANSI code page
        assert_eq!(hc.text_type.code_page(932), 932);
    }

    #[test]
    fn parse_case_insensitive_type() {
        let hc = parse_h_code("/Hw-4@12345:game.exe").unwrap();
        assert_eq!(hc.text_type, HookTextType::Wide);
    }

    #[test]
    fn parse_0x_prefix_offset() {
        let hc = parse_h_code("/HA0x10@12345:game.exe").unwrap();
        assert_eq!(hc.data_offset, 0x10);
    }

    #[test]
    fn parse_positive_offset_with_plus() {
        let hc = parse_h_code("/HA+10@12345:game.exe").unwrap();
        assert_eq!(hc.data_offset, 0x10);
    }

    #[test]
    fn parse_missing_prefix_fails() {
        assert_eq!(parse_h_code("XA-4@12345:game.exe").unwrap_err(),
                   HookCodeError::MissingPrefix);
    }

    #[test]
    fn parse_missing_at_fails() {
        assert_eq!(parse_h_code("/HA-4").unwrap_err(),
                   HookCodeError::MissingAtModule);
    }

    #[test]
    fn parse_missing_module_fails() {
        assert_eq!(parse_h_code("/HA-4@12345:").unwrap_err(),
                   HookCodeError::EmptyModule);
    }

    #[test]
    fn parse_invalid_addr_fails() {
        assert_eq!(parse_h_code("/HA-4@nothex:game.exe").unwrap_err(),
                   HookCodeError::InvalidAddress);
    }

    #[test]
    fn parse_invalid_offset_fails() {
        assert_eq!(parse_h_code("/HAxyz@12345:game.exe").unwrap_err(),
                   HookCodeError::InvalidOffset("xyz".to_string()));
    }

    #[test]
    fn parse_empty_offset_defaults_to_zero() {
        let hc = parse_h_code("/HA@12345:game.exe").unwrap();
        assert_eq!(hc.data_offset, 0);
    }

    #[test]
    fn is_rva_threshold() {
        assert!(is_rva(0xFFFF));
        assert!(!is_rva(0x10000));
        assert!(!is_rva(0xDEADBEEF));
    }

    #[test]
    fn code_page_mapping() {
        assert_eq!(HookTextType::Wide.code_page(932), 0);
        assert_eq!(HookTextType::Ansi.code_page(932), 932);
        assert_eq!(HookTextType::Ansi.code_page(936), 936);
        assert_eq!(HookTextType::NullTerminated.code_page(949), 949); // Korean
    }

    #[test]
    fn round_trip_display() {
        // Verify Display impl works for error messages
        let err = HookCodeError::InvalidOffset("xyz".to_string());
        assert!(err.to_string().contains("xyz"));
    }
}
