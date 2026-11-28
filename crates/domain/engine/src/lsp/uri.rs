//! `file:` URIs to paths and back, for both Windows and POSIX path shapes.
//!
//! Windows paths become `file:///C:/dir/name` (drive letter upper case) or `file://host/share/name` for UNC
//! paths; verbatim `\\?\` prefixes are dropped. Decoding accepts the variants
//! servers send, such as an encoded drive colon (`file:///c%3A/...`), `localhost` as the host, and a query or fragment. A URI the platform cannot
//! name (another scheme, or a remote host on POSIX) yields `None`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::lsp::platform;

/// The path shape a URI is converted from or to, independent of the host
/// platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStyle {
    Windows,
    Posix,
}

impl PathStyle {
    pub const NATIVE: Self = platform::PATH_STYLE;
}

pub fn path_to_uri(path: &Path) -> String {
    encode_path(&path.to_string_lossy(), PathStyle::NATIVE)
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    decode_uri(uri, PathStyle::NATIVE).map(PathBuf::from)
}

pub fn encode_path(path: &str, style: PathStyle) -> String {
    match style {
        PathStyle::Posix => {
            if path.starts_with('/') {
                format!("file://{}", percent_encode(path))
            } else {
                format!("file:///{}", percent_encode(path))
            }
        },
        PathStyle::Windows => encode_windows(path),
    }
}

fn encode_windows(path: &str) -> String {
    let backslashed = path.replace('/', "\\");
    let plain = backslashed
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| backslashed.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or(backslashed);
    if let Some(unc) = plain.strip_prefix(r"\\") {
        let (host, rest) = unc.split_once('\\').unwrap_or((unc, ""));
        return format!(
            "file://{}/{}",
            percent_encode(host),
            percent_encode(&rest.replace('\\', "/"))
        );
    }
    let mut slashed = upper_drive(&plain.replace('\\', "/"));
    if slashed.len() == 2 && has_drive(&slashed) {
        slashed.push('/');
    }
    if slashed.starts_with('/') {
        format!("file://{}", percent_encode(&slashed))
    } else {
        format!("file:///{}", percent_encode(&slashed))
    }
}

pub fn decode_uri(uri: &str, style: PathStyle) -> Option<String> {
    let (scheme, rest) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (authority, raw_path) = rest.strip_prefix("//").map_or(("", rest), |after| {
        after
            .find('/')
            .map_or((after, "/"), |index| after.split_at(index))
    });
    let path = percent_decode(raw_path)?;
    let host = if authority.eq_ignore_ascii_case("localhost") {
        String::new()
    } else {
        percent_decode(authority)?
    };
    match style {
        PathStyle::Posix => {
            if !host.is_empty() {
                return None;
            }
            if path.starts_with('/') {
                Some(path)
            } else {
                Some(format!("/{path}"))
            }
        },
        PathStyle::Windows => {
            if !host.is_empty() {
                return Some(format!(r"\\{host}{}", path.replace('/', "\\")));
            }
            let local = match path.strip_prefix('/') {
                Some(rest) if has_drive(rest) => rest,
                _ => path.as_str(),
            };
            let mut native = upper_drive(&local.replace('/', "\\"));
            if native.len() == 2 && has_drive(&native) {
                native.push('\\');
            }
            Some(native)
        },
    }
}

pub(crate) const fn has_drive(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub(crate) fn upper_drive(text: &str) -> String {
    if has_drive(text) {
        let mut out = String::with_capacity(text.len());
        out.push(char::from(text.as_bytes()[0].to_ascii_uppercase()));
        out.push_str(&text[1..]);
        out
    } else {
        text.to_owned()
    }
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";

fn keep_unencoded(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~/:!$&'()*+,;=@".contains(&byte)
}

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if keep_unencoded(byte) {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
    }
    out
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&byte) = bytes.get(index) {
        if byte == b'%' {
            let high = bytes.get(index + 1).copied().and_then(hex_value);
            let low = bytes.get(index + 2).copied().and_then(hex_value);
            if let (Some(high), Some(low)) = (high, low) {
                out.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        out.push(byte);
        index += 1;
    }
    String::from_utf8(out).ok()
}
