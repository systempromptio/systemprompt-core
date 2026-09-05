//! Delta, item-completion and terminal event handling for the Responses SSE
//! stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::Value;

use crate::wire::canonical::{CanonicalEvent, CanonicalUsageUpdate};
use crate::wire::openai_responses::slot::{
    ResponsesStreamState, SlotKindMatch, lookup_canonical, stop_reason,
};

#[derive(Clone, Copy)]
pub(super) enum DeltaShape {
    Text,
    ToolUse,
    Thinking,
}

pub(super) fn emit_delta(
    state: &ResponsesStreamState,
    value: &Value,
    want: SlotKindMatch,
    events: &mut Vec<Result<CanonicalEvent, String>>,
    shape: DeltaShape,
) {
    let output_index = value
        .get("output_index")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let Some(idx) = lookup_canonical(&state.items, output_index, want) else {
        return;
    };
    let delta = value.get("delta").and_then(Value::as_str).unwrap_or("");
    if delta.is_empty() {
        return;
    }
    let event = match shape {
        DeltaShape::Text => CanonicalEvent::TextDelta {
            index: idx,
            text: delta.to_owned(),
        },
        DeltaShape::ToolUse => CanonicalEvent::ToolUseDelta {
            index: idx,
            partial_json: delta.to_owned(),
        },
        DeltaShape::Thinking => CanonicalEvent::ThinkingDelta {
            index: idx,
            text: delta.to_owned(),
        },
    };
    events.push(Ok(event));
}

pub(super) fn handle_item_done(
    state: &ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
) {
    let output_index = value
        .get("output_index")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if let Some(slot) = state.items.iter().find(|s| s.output_index == output_index) {
        if let Some(encrypted) = value
            .get("item")
            .and_then(|i| i.get("encrypted_content"))
            .and_then(Value::as_str)
        {
            events.push(Ok(CanonicalEvent::EncryptedContentDelta {
                index: slot.canonical_index,
                data: encrypted.to_owned(),
            }));
        }
        events.push(Ok(CanonicalEvent::ContentBlockStop {
            index: slot.canonical_index,
        }));
    }
}

pub(super) fn handle_completed(
    state: &ResponsesStreamState,
    value: &Value,
    events: &mut Vec<Result<CanonicalEvent, String>>,
    incomplete: bool,
) {
    let response = value.get("response").unwrap_or(&Value::Null);
    let id = response
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map_or_else(|| state.response_id.clone(), str::to_owned);
    if let Some(usage) = response.get("usage") {
        let pull = |key: &str| usage.get(key).and_then(Value::as_u64).map(|v| v as u32);
        let cached = usage
            .get("input_tokens_details")
            .and_then(|d| d.get("cached_tokens"))
            .and_then(Value::as_u64)
            .map(|v| v as u32);
        // Why: `cached_tokens` is a subset of `input_tokens` here, but
        // `CanonicalUsage::input_tokens` is exclusive of cache reads, so the
        // streamed frame must subtract exactly as the buffered parse does or
        // the same reply prices differently on the two paths.
        events.push(Ok(CanonicalEvent::UsageDelta(CanonicalUsageUpdate {
            input_tokens: pull("input_tokens")
                .map(|input| input.saturating_sub(cached.unwrap_or(0))),
            output_tokens: pull("output_tokens"),
            cache_read_tokens: cached,
            cache_creation_tokens: None,
            total_tokens: pull("total_tokens"),
            // Why: already inside `output_tokens` on this contract, so it is
            // reported as a breakdown and never added to the total.
            reasoning_tokens: usage
                .get("output_tokens_details")
                .and_then(|d| d.get("reasoning_tokens"))
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        })));
    }
    let incomplete_reason = incomplete
        .then(|| {
            response
                .get("incomplete_details")
                .and_then(|d| d.get("reason"))
                .and_then(Value::as_str)
        })
        .flatten();
    events.push(Ok(CanonicalEvent::MessageStop {
        id,
        stop_reason: Some(stop_reason(&state.items, incomplete_reason)),
    }));
}

pub(super) fn handle_error(value: &Value, events: &mut Vec<Result<CanonicalEvent, String>>) {
    let msg = value
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("upstream error")
        .to_owned();
    events.push(Ok(CanonicalEvent::Error(msg)));
}
