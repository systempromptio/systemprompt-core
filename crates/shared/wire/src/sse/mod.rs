//! Server-Sent Events framing shared by every provider SSE codec.
//!
//! [`SseFramer`] is the one incremental decoder every codec and every
//! passthrough lane reads upstream bytes through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod framer;

pub use framer::{MAX_SSE_FRAME_BYTES, SseFrame, SseFrameTooLarge, SseFramer};

// Why: a serde message can quote the offending value, and payload text must
// never reach the logs.
pub(crate) fn skip_undecodable_json(wire: &'static str, error: &serde_json::Error, bytes: usize) {
    let reason = format!(
        "{:?} error at line {} column {}",
        error.classify(),
        error.line(),
        error.column()
    );
    skip_frame(wire, &reason, bytes);
}

pub(crate) fn skip_frame(wire: &'static str, reason: &str, bytes: usize) {
    tracing::warn!(
        wire,
        reason,
        bytes,
        "skipped undecodable upstream SSE frame"
    );
}

// Why: Providers can signal mid-stream failure with an `{"error": {...}}` chunk
// after the HTTP response has already returned 200.
// JSON: Upstream provider error body; every vendor uses a different shape.
pub fn upstream_error_message(value: &serde_json::Value) -> Option<String> {
    let error = value.get("error")?;
    if error.is_null() {
        return None;
    }
    if let Some(message) = error.as_str() {
        return Some(message.to_owned());
    }
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("upstream error");
    // Why: Google's `{code, status, message}` shape names the failure class in
    // `status`; without it "Invalid value at contents[1]…" reads as ours.
    let code = error.get("code").and_then(serde_json::Value::as_u64);
    let status = error
        .get("status")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty());
    Some(match (code, status) {
        (Some(code), Some(status)) => format!("upstream {code} {status}: {message}"),
        (Some(code), None) => format!("upstream {code}: {message}"),
        (None, Some(status)) => format!("upstream {status}: {message}"),
        (None, None) => message.to_owned(),
    })
}
