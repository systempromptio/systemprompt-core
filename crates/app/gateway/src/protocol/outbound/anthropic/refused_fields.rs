//! Request body fields an upstream has refused, learned per provider.
//!
//! An upstream that does not know a top-level body field refuses the request
//! with a 400 naming it: "`context_management`: Extra inputs are not permitted"
//! (Vertex AI, and Anthropic's own validator). That is the failure a client
//! beta gets when the gateway forwards the field but not the flag that gates
//! it — or when the upstream serves neither. The known pairs are removed
//! before the request leaves (`BETA_GATED_FIELDS`); this learner is the net
//! under them: the named field is dropped, the request re-sent once, and the
//! provider remembers it. A field the Messages API requires is never dropped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::learned::Learned;

const REFUSAL_MARKER: &str = "Extra inputs are not permitted";

// Why: a refusal that names one of these is a malformed request, not an
// unsupported feature; dropping it would send an even more broken request.
const NEVER_DROPPED: &[&str] = &[
    "anthropic_version",
    "max_tokens",
    "messages",
    "metadata",
    "model",
    "stop_sequences",
    "stream",
    "system",
    "temperature",
    "thinking",
    "tool_choice",
    "tools",
    "top_k",
    "top_p",
];

static LEARNED: Learned = Learned::new();

#[must_use]
pub fn refused_in(message: &str) -> BTreeSet<String> {
    let mut chunks: Vec<&str> = message.split(REFUSAL_MARKER).collect();
    chunks.pop();
    chunks
        .into_iter()
        .filter_map(top_level_field_named_by)
        .filter(|field| !NEVER_DROPPED.contains(&field.as_str()))
        .collect()
}

// Why: pydantic phrases a refusal as "<path>: Extra inputs are not permitted",
// with a dotted path for a nested field; only its first segment is a body key.
fn top_level_field_named_by(before: &str) -> Option<String> {
    let path = before.trim_end().strip_suffix(':')?.trim_end();
    let path = path
        .rsplit(|c: char| c.is_whitespace() || c == '`' || c == '"' || c == '\'')
        .next()?;
    let top = path.split('.').next()?;
    let valid = !top.is_empty()
        && top.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
        && top
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    valid.then(|| top.to_owned())
}

#[must_use]
pub fn learned(provider: &str) -> BTreeSet<String> {
    LEARNED.for_provider(provider)
}

pub fn learn(provider: &str, refused: &BTreeSet<String>) {
    LEARNED.learn(provider, refused);
}
