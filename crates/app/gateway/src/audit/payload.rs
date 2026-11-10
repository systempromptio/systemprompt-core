//! Capped request/response payload capture for gateway audit rows.
//!
//! The cap is `governance.audit.payload_cap_bytes` from the profile
//! ([`AuditConfig`]); a body at or under it is stored whole, over it only the
//! digest and a head+tail excerpt survive. A probe (`max_tokens <= 1`) is
//! always excerpt-only: its body is the prompt the next real turn re-sends,
//! so storing it in full serves no reader.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};
use systemprompt_manifest::profile::AuditConfig;

const EXCERPT_BYTES: usize = 8 * 1024;

/// What a captured request or response body contributes to an audit row.
///
/// `sha256` is computed over the **full** bytes the gateway held, so a capped
/// capture still proves which body was sent. A streamed response longer than
/// the tap's retention is held only as a prefix: its capture is `truncated`,
/// `byte_len` is the full streamed length, and the excerpt names the retained
/// length the digest covers.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PayloadCapture {
    // JSON: audited request/response body — captured verbatim from the provider wire.
    pub json: Option<Value>,
    pub excerpt: Option<String>,
    pub truncated: bool,
    pub byte_len: i32,
    pub sha256: String,
}

#[must_use]
pub fn digest_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// JSON: provider wire `tools` array — audited verbatim in the client's protocol
// shape.
#[must_use]
pub fn tools_array(body: &[u8]) -> Option<Value> {
    let mut parsed = serde_json::from_slice::<Value>(body).ok()?;
    let tools = parsed.get_mut("tools")?.take();
    tools.is_array().then_some(tools)
}

// JSON: provider wire `tools` array — audited verbatim in the upstream protocol
// shape.
#[must_use]
pub fn prepared_tools(body: &[u8]) -> Option<Value> {
    tools_array(body)
}

#[must_use]
pub fn excerpt_payload(bytes: &Bytes) -> PayloadCapture {
    let len = bytes.len();
    let head_len = EXCERPT_BYTES.min(len);
    let head = String::from_utf8_lossy(&bytes[..head_len]).to_string();
    let tail_start = len.saturating_sub(EXCERPT_BYTES).max(head_len);
    let tail = String::from_utf8_lossy(&bytes[tail_start..]).to_string();
    let dropped = tail_start - head_len;
    let excerpt = if dropped == 0 && tail.is_empty() {
        head
    } else {
        format!("{head}\n...<truncated {dropped} bytes>...\n{tail}")
    };
    PayloadCapture {
        json: None,
        excerpt: Some(excerpt),
        truncated: true,
        byte_len: len.min(i32::MAX as usize) as i32,
        sha256: digest_hex(bytes),
    }
}

#[must_use]
pub fn slice_payload(bytes: &Bytes, cap_bytes: usize) -> PayloadCapture {
    let cap = cap_bytes.max(AuditConfig::MIN_PAYLOAD_CAP_BYTES);
    let len = bytes.len();
    let byte_len = len.min(i32::MAX as usize) as i32;
    let sha256 = digest_hex(bytes);
    if len <= cap {
        serde_json::from_slice::<Value>(bytes).map_or_else(
            |_| PayloadCapture {
                json: None,
                excerpt: Some(String::from_utf8_lossy(bytes).to_string()),
                truncated: false,
                byte_len,
                sha256: sha256.clone(),
            },
            |v| PayloadCapture {
                json: Some(v),
                excerpt: None,
                truncated: false,
                byte_len,
                sha256: sha256.clone(),
            },
        )
    } else {
        excerpt_payload(bytes)
    }
}

/// A streamed response as the tap ends it: the body it retained, the full
/// length that was streamed, and whether its usage was reported or settled to
/// the admission reservation.
#[derive(Debug, Clone, Copy)]
pub struct StreamedResponse<'a> {
    pub bytes: &'a Bytes,
    pub streamed_bytes: u64,
    pub usage_source: systemprompt_ai::repository::ai_requests::UsageSource,
}

#[must_use]
pub fn slice_retained_payload(
    retained: &Bytes,
    streamed_bytes: u64,
    cap_bytes: usize,
) -> PayloadCapture {
    let retained_len = u64::try_from(retained.len()).unwrap_or(u64::MAX);
    if streamed_bytes <= retained_len {
        return slice_payload(retained, cap_bytes);
    }
    let mut capture = excerpt_payload(retained);
    capture.byte_len = i32::try_from(streamed_bytes).unwrap_or(i32::MAX);
    capture.excerpt = capture.excerpt.map(|excerpt| {
        format!(
            "<retained {retained_len} of {streamed_bytes} bytes; sha256 covers the retained \
             prefix only>\n{excerpt}"
        )
    });
    capture
}

pub fn truncate_for_tool_input(input: &str) -> String {
    const TOOL_INPUT_CAP: usize = 64 * 1024;
    if input.len() <= TOOL_INPUT_CAP {
        input.to_owned()
    } else {
        let cut = systemprompt_models::text::floor_char_boundary(input, TOOL_INPUT_CAP);
        let head = &input[..cut];
        format!("{head}...<truncated {} bytes>", input.len() - cut)
    }
}
