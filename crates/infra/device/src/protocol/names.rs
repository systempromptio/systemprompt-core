//! Names several crates derive from the same input and must agree on byte for
//! byte.
//!
//! A session UUID is the RFC 4122 v5 UUID of the native session id under
//! [`SESSION_NAMESPACE`](crate::protocol::names::SESSION_NAMESPACE), so every
//! client of an instance derives the same one for hook events and model
//! metadata; the instance knows the session by that
//! UUID ([`native_session_id`](crate::protocol::names::native_session_id)). An
//! MCP server slug keeps lowercase `[a-z0-9_]` and folds every other run into
//! one `-`; tool names are `<slug>_<tool>`, so skills and rules that name a
//! tool depend on it. A skill directory keeps lowercase ASCII alphanumerics, at
//! most 64, and is empty when nothing is left. Plain ids, bundle paths and
//! plugin segments are checked before they touch a file system. Fingerprints
//! are hex SHA-256: of the raw ed25519 key bytes (prefixed `sha256:`), and of
//! the install id and user id under a fixed domain line for a device.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::protocol::error::PubkeyError;
use systemprompt_identifiers::{NativeSessionId, UserId};

use crate::protocol::ids::InstallId;

pub const SESSION_NAMESPACE: Uuid = Uuid::from_u128(0x7c1f_5b6e_3a2d_4e8f_9b0c_2d6a_1e4f_8c73);

#[must_use]
pub fn session_uuid(native: &str) -> Uuid {
    Uuid::new_v5(&SESSION_NAMESPACE, native.as_bytes())
}

#[must_use]
pub fn session_uuid_string(native: &str) -> String {
    session_uuid(native).hyphenated().to_string()
}

#[must_use]
pub fn native_session_id(native: &str) -> NativeSessionId {
    NativeSessionId::new(session_uuid_string(native))
}

#[must_use]
pub fn canonical_session_uuid(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw)
        .ok()
        .filter(|id| id.hyphenated().to_string() == raw)
}

#[must_use]
pub fn mcp_server_slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut after_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c.to_ascii_lowercase());
            after_dash = false;
        } else if !after_dash {
            out.push('-');
            after_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "mcp-server".to_owned();
    }
    out
}

pub const SKILL_DIR_MAX_LEN: usize = 64;

#[must_use]
pub fn skill_dir_name(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut after_dash = true;
    for c in id.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            after_dash = false;
        } else if !after_dash {
            out.push('-');
            after_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(SKILL_DIR_MAX_LEN);
    while out.ends_with('-') {
        out.pop();
    }
    out
}

pub const PLAIN_ID_MAX_LEN: usize = 64;

#[must_use]
pub fn is_plain_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    match bytes.split_first() {
        Some((first, rest)) => {
            bytes.len() <= PLAIN_ID_MAX_LEN
                && first.is_ascii_alphanumeric()
                && rest
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        },
        None => false,
    }
}

#[must_use]
pub fn is_plain_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[must_use]
pub fn is_safe_plugin_segment(plugin: &str) -> bool {
    !plugin.is_empty()
        && !plugin.contains("..")
        && !plugin.contains(['/', '\\'])
        && !plugin.starts_with('.')
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(Sha256::digest(bytes).as_slice())
}

#[must_use]
pub fn key_fingerprint(raw_key: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(raw_key))
}

pub fn decode_pubkey(pubkey_b64: &str) -> Result<[u8; 32], PubkeyError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(pubkey_b64.trim())
        .map_err(PubkeyError::Base64)?;
    let length = bytes.len();
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_mismatch| PubkeyError::Length(length))
}

pub fn pubkey_fingerprint(pubkey_b64: &str) -> Result<String, PubkeyError> {
    decode_pubkey(pubkey_b64).map(|raw| key_fingerprint(&raw))
}

const DEVICE_FINGERPRINT_DOMAIN: &str = "systemprompt-bridge-device/v1";

#[must_use]
pub fn device_fingerprint(install_id: &InstallId, user_id: &UserId) -> String {
    sha256_hex(
        format!(
            "{DEVICE_FINGERPRINT_DOMAIN}\n{}\n{}",
            install_id.as_str(),
            user_id.as_str()
        )
        .as_bytes(),
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}
