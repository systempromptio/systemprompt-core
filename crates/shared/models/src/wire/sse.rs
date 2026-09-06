//! Server-Sent Events framing shared by every provider SSE codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub fn frame_end(buf: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < buf.len() {
        let Some(first) = newline_len(buf, i) else {
            i += 1;
            continue;
        };
        let after_first = i + first;
        if let Some(second) = newline_len(buf, after_first) {
            return Some(after_first + second);
        }
        i = after_first;
    }
    None
}

fn newline_len(buf: &[u8], idx: usize) -> Option<usize> {
    match buf.get(idx) {
        Some(b'\r') if buf.get(idx + 1) == Some(&b'\n') => Some(2),
        Some(b'\n' | b'\r') => Some(1),
        _ => None,
    }
}

// Why: providers signal a mid-stream failure by sending an `{"error": {...}}`
// object in place of a normal chunk, on a connection that already returned
// 200. A codec that only knows the success shape parses that as an empty
// chunk and the stream simply ends, so the caller cannot tell a failure from
// a hang. Every codec routes candidate chunks through here first.
pub fn upstream_error_message(value: &serde_json::Value) -> Option<String> {
    let error = value.get("error")?;
    if error.is_null() {
        return None;
    }
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("upstream error");
    Some(message.to_owned())
}
