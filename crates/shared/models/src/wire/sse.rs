//! Server-Sent Events framing shared by every provider SSE codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Returns the byte length of the first complete SSE event in `buf`, including
/// its terminating blank line, or `None` if no complete event has arrived yet.
///
/// Recognises `\n\n`, `\r\n\r\n`, `\r\r`, and mixed pairings such as `\n\r\n`,
/// so codecs stay agnostic to the upstream's choice of line ending.
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
