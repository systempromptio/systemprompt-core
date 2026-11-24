//! Text-formatting helpers.
//!
//! [`truncate_with_ellipsis`] shortens a string to a maximum byte
//! length, appending `...` and snapping to a UTF-8 character boundary
//! so the result is never split mid-codepoint. [`chunk_text`] splits long
//! output into line-aligned chunks under a byte limit, shared by the Slack
//! Block Kit and Teams Adaptive Card renderers. [`without_nul`] and
//! [`json_without_nul`] make text and JSON storable in `PostgreSQL`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use serde_json::Value;

const NUL_REPLACEMENT: &str = "\u{FFFD}";

// Why: PostgreSQL stores no NUL in `text` (SQLSTATE 22021) or `jsonb`
// (SQLSTATE 22P05), so a value bound to either carries U+FFFD in its place.
#[must_use]
pub fn without_nul(text: &str) -> Cow<'_, str> {
    if text.contains('\0') {
        Cow::Owned(text.replace('\0', NUL_REPLACEMENT))
    } else {
        Cow::Borrowed(text)
    }
}

// JSON: any document bound to a JSONB column; string leaves and object keys
// are rewritten, the structure is kept.
#[must_use]
pub fn json_without_nul(value: &Value) -> Cow<'_, Value> {
    if !json_has_nul(value) {
        return Cow::Borrowed(value);
    }
    let mut owned = value.clone();
    replace_json_nul(&mut owned);
    Cow::Owned(owned)
}

// JSON: walks the same arbitrary document as `json_without_nul`.
fn json_has_nul(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains('\0'),
        Value::Array(items) => items.iter().any(json_has_nul),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, field)| key.contains('\0') || json_has_nul(field)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

// JSON: walks the same arbitrary document as `json_without_nul`.
fn replace_json_nul(value: &mut Value) {
    match value {
        Value::String(text) => {
            if text.contains('\0') {
                *text = text.replace('\0', NUL_REPLACEMENT);
            }
        },
        Value::Array(items) => items.iter_mut().for_each(replace_json_nul),
        Value::Object(fields) => {
            *fields = std::mem::take(fields)
                .into_iter()
                .map(|(key, mut field)| {
                    replace_json_nul(&mut field);
                    (without_nul(&key).into_owned(), field)
                })
                .collect();
        },
        Value::Null | Value::Bool(_) | Value::Number(_) => {},
    }
}

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
    let boundary = floor_char_boundary(text, truncated_len);
    format!("{}...", &text[..boundary])
}

#[must_use]
pub const fn floor_char_boundary(s: &str, target: usize) -> usize {
    if target >= s.len() {
        return s.len();
    }

    let mut boundary = target;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}
