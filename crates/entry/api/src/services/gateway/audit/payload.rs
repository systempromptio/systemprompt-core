//! Capped request/response payload capture for gateway audit rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAYLOAD_CAP_BYTES: usize = 1024 * 1024;
const EXCERPT_BYTES: usize = 8 * 1024;

/// What a captured request or response body contributes to an audit row.
///
/// `sha256` is computed over the **full** bytes regardless of truncation, so a
/// capped capture still proves which body was sent.
#[derive(Debug, Clone)]
pub struct PayloadCapture {
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

#[must_use]
pub fn slice_payload(bytes: &Bytes) -> PayloadCapture {
    let len = bytes.len();
    let byte_len = len.min(i32::MAX as usize) as i32;
    let sha256 = digest_hex(bytes);
    if len <= PAYLOAD_CAP_BYTES {
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
        let head_len = EXCERPT_BYTES.min(len);
        let head = String::from_utf8_lossy(&bytes[..head_len]).to_string();
        let tail_start = len.saturating_sub(EXCERPT_BYTES);
        let tail_len = len - tail_start;
        let tail = String::from_utf8_lossy(&bytes[tail_start..]).to_string();
        let dropped = len - head_len - tail_len;
        let excerpt = format!("{head}\n...<truncated {dropped} bytes>...\n{tail}");
        PayloadCapture {
            json: None,
            excerpt: Some(excerpt),
            truncated: true,
            byte_len,
            sha256,
        }
    }
}

pub fn truncate_for_tool_input(input: &str) -> String {
    const TOOL_INPUT_CAP: usize = 64 * 1024;
    if input.len() <= TOOL_INPUT_CAP {
        input.to_owned()
    } else {
        // Why: `&input[..TOOL_INPUT_CAP]` panics when the cap lands inside a
        // multi-byte UTF-8 codepoint. Walk back to the nearest char boundary
        // before slicing so non-ASCII tool inputs cannot crash audit logging.
        let mut cut = TOOL_INPUT_CAP;
        while cut > 0 && !input.is_char_boundary(cut) {
            cut -= 1;
        }
        let head = &input[..cut];
        format!("{head}...<truncated {} bytes>", input.len() - cut)
    }
}
