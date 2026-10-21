//! Strict tool-call pairing check for an `OpenAI` Chat `messages` array.
//!
//! The passthrough lane forwards the client's own `messages` only when every
//! assistant `tool_calls` entry is answered by the `role: "tool"` messages
//! that immediately follow it, and no tool message answers a call that is not
//! open; otherwise the body is rebuilt from the normalised canonical history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic JSON.
use serde_json::Value;

#[must_use]
pub fn history_is_strict(messages: &Value) -> bool {
    let Some(messages) = messages.as_array() else {
        return false;
    };
    let mut pending: BTreeSet<&str> = BTreeSet::new();
    for message in messages {
        match message.get("role").and_then(Value::as_str) {
            Some("tool") => {
                let id = message
                    .get("tool_call_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if !pending.remove(id) {
                    return false;
                }
            },
            Some(role) => {
                if !pending.is_empty() {
                    return false;
                }
                if role == "assistant" {
                    pending = message
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .map(|calls| {
                            calls
                                .iter()
                                .filter_map(|c| c.get("id").and_then(Value::as_str))
                                .collect()
                        })
                        .unwrap_or_default();
                }
            },
            None => return false,
        }
    }
    pending.is_empty()
}
