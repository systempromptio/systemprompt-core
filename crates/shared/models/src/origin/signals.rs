//! The wire signals below a declaration: a harness's native marker in the
//! request body and the first `User-Agent` product token.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ClientKind, NativeMarker};

// JSON: protocol boundary — the inference body is any host's wire shape.
#[must_use]
pub fn native_marker(body: &[u8]) -> Option<NativeMarker> {
    if body.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if value
        .pointer("/client_metadata/x-codex-turn-metadata")
        .is_some()
    {
        return Some(NativeMarker::CodexTurnMetadata);
    }
    claude_entrypoint(&value).or_else(|| claude_metadata_user_id(&value))
}

// Why: Claude Code puts the billing header in the first system block on a
// third-party gateway; a Cowork session runs the same runtime as the CLI and
// this prefix is the one place the body says which of the two it is.
// JSON: Anthropic Messages request — probed for Claude Code markers.
fn claude_entrypoint(value: &serde_json::Value) -> Option<NativeMarker> {
    let system = value.get("system")?;
    let text = match system {
        serde_json::Value::String(text) => text.as_str(),
        serde_json::Value::Array(blocks) => blocks.first()?.get("text")?.as_str()?,
        _ => return None,
    };
    let rest = text
        .trim_start()
        .strip_prefix("x-anthropic-billing-header:")?;
    let entrypoint = rest
        .split(';')
        .map(str::trim)
        .find_map(|field| field.strip_prefix("cc_entrypoint="))?;
    match entrypoint {
        "cli" => Some(NativeMarker::ClaudeCliEntrypoint),
        "local-agent" => Some(NativeMarker::ClaudeDesktopEntrypoint),
        entry if entry.starts_with(ClientKind::ClaudeDesktop.as_str()) => {
            Some(NativeMarker::ClaudeDesktopEntrypoint)
        },
        _ => None,
    }
}

// JSON: Anthropic Messages request — probed for Claude Code markers.
fn claude_metadata_user_id(value: &serde_json::Value) -> Option<NativeMarker> {
    let user_id = value.pointer("/metadata/user_id")?.as_str()?.trim();
    if user_id.starts_with('{') {
        // JSON: protocol boundary — Claude Code ≥ 2.1.25x stamps its session
        // as a JSON string; `device_id` beside `session_id` is the shape.
        let metadata: serde_json::Value = serde_json::from_str(user_id).ok()?;
        return (metadata
            .get("device_id")
            .is_some_and(serde_json::Value::is_string)
            && metadata
                .get("session_id")
                .is_some_and(serde_json::Value::is_string))
        .then_some(NativeMarker::ClaudeMetadataJson);
    }
    // Why: `user_<hex>_account_<uuid>_session_<uuid>` is the grammar Claude
    // Code and Claude Desktop stamp; the checks are structural, not a match on
    // any client-chosen text.
    let mut parts = user_id.split('_');
    let shape = [
        parts.next() == Some("user"),
        parts.next().is_some_and(|hex| !hex.is_empty()),
        parts.next() == Some("account"),
        parts
            .next()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()),
        parts.next() == Some("session"),
        parts
            .next()
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()),
        parts.next().is_none(),
    ];
    shape
        .iter()
        .all(|ok| *ok)
        .then_some(NativeMarker::ClaudeMetadataUserId)
}

// Why: the first product token is the only part of a User-Agent with a
// defined grammar (RFC 9110 §10.1.5); everything after it is free text and is
// never matched.
#[must_use]
pub fn ua_product(user_agent: Option<&str>) -> Option<(String, Option<String>)> {
    let first = user_agent?.split_ascii_whitespace().next()?;
    let (product, version) = first
        .split_once('/')
        .map_or((first, None), |(p, v)| (p, Some(v)));
    if product.is_empty() || !product.chars().all(is_token_char) {
        return None;
    }
    Some((
        product.to_ascii_lowercase(),
        version.filter(|v| !v.is_empty()).map(str::to_owned),
    ))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
}
