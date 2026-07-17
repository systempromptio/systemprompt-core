//! Text-formatting helpers.
//!
//! [`truncate_with_ellipsis`] shortens a string to a maximum byte
//! length, appending `...` and snapping to a UTF-8 character boundary
//! so the result is never split mid-codepoint. [`chunk_text`] splits long
//! output into line-aligned chunks under a byte limit, shared by the Slack
//! Block Kit and Teams Adaptive Card renderers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Split `text` into chunks each under `limit` bytes, breaking on line
/// boundaries.
///
/// A chunk never splits a line. An empty input yields a single empty chunk
/// (callers render it as one empty block). A single line longer than `limit`
/// is emitted whole rather than cut mid-line.
#[must_use]
pub fn chunk_text(text: &str, limit: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if current.len() + line.len() + 1 > limit && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

pub fn truncate_with_ellipsis(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_owned();
    }

    let truncated_len = max_len.saturating_sub(3);
    let boundary = find_char_boundary(text, truncated_len);
    format!("{}...", &text[..boundary])
}

const fn find_char_boundary(s: &str, target: usize) -> usize {
    if target >= s.len() {
        return s.len();
    }

    let mut boundary = target;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}
