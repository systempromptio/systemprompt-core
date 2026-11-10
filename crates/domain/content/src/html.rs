//! HTML output encoding for component renderers that build markup directly:
//! text/attribute escaping, URL scheme filtering and path-segment encoding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const HEX: &[u8; 16] = b"0123456789ABCDEF";
const BLOCKED_SCHEMES: [&str; 3] = ["javascript:", "vbscript:", "data:"];

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            '`' => out.push_str("&#x60;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn safe_url(value: &str) -> Option<&str> {
    let normalized: String = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .take(16)
        .collect::<String>()
        .to_ascii_lowercase();
    if BLOCKED_SCHEMES.iter().any(|s| normalized.starts_with(s)) {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    out
}
