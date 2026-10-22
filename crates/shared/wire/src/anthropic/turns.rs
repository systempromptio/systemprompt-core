//! Strict-alternation check and rendering for an Anthropic `messages` array.
//!
//! The Messages API merges consecutive same-role turns; Anthropic-compatible
//! upstreams (`DeepSeek`, `MiniMax`) do not, and they reject a `tool_use` not
//! answered by the very next turn. The passthrough lane forwards the client's
//! own `messages` only when they already hold that shape, and otherwise
//! re-renders them from the normalised canonical history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::Value;

use super::blocks::{BlockAudience, canonical_message_to_anthropic};
use crate::canonical::{CanonicalMessage, TurnMerge, normalize_turns};

#[must_use]
// JSON: Anthropic Messages API `messages` entries; upstream JSON is the
// contract.
pub fn upstream_messages(messages: &[CanonicalMessage]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for message in normalize_turns(messages, TurnMerge::UserSide)
        .iter()
        .filter_map(|m| canonical_message_to_anthropic(m, BlockAudience::Upstream))
    {
        // Why: an assistant turn holding only unsigned thinking renders to
        // nothing upstream, which would leave its neighbours as two user turns.
        match out.last_mut() {
            Some(last) if last.get("role") == message.get("role") => {
                if let (Some(into), Some(from)) = (
                    last.get_mut("content").and_then(Value::as_array_mut),
                    message.get("content").and_then(Value::as_array),
                ) {
                    into.extend(from.iter().cloned());
                }
            },
            _ => out.push(message),
        }
    }
    out
}

#[must_use]
// JSON: the client's own Anthropic `messages` array, checked before
// passthrough.
pub fn history_is_strict(messages: &Value) -> bool {
    let Some(messages) = messages.as_array() else {
        return false;
    };
    let mut previous_role: Option<&str> = None;
    let mut pending: BTreeSet<&str> = BTreeSet::new();
    for message in messages {
        let Some(role) = message.get("role").and_then(Value::as_str) else {
            return false;
        };
        if previous_role == Some(role) {
            return false;
        }
        let blocks = blocks(message);
        let ids = |kind: &str, key: &str| -> BTreeSet<&str> {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some(kind))
                .filter_map(|b| b.get(key).and_then(Value::as_str))
                .collect()
        };
        let answered = ids("tool_result", "tool_use_id");
        if role == "user" {
            if answered != pending {
                return false;
            }
            pending.clear();
        } else {
            if !pending.is_empty() || !answered.is_empty() {
                return false;
            }
            pending = ids("tool_use", "id");
        }
        previous_role = Some(role);
    }
    true
}

// JSON: one Anthropic Messages API message; upstream JSON is the contract.
fn blocks(message: &Value) -> &[Value] {
    message
        .get("content")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}
