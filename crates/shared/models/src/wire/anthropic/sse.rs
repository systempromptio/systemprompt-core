//! Anthropic Messages streaming-frame parse side of the codec.
//!
//! [`AnthropicStreamState::events_from_sse`] turns one decoded SSE `data:`
//! payload into zero or more [`CanonicalEvent`]s. The streaming side stays
//! dynamic because each frame is a distinct, sparsely-populated event keyed on
//! `type`.
//!
//! The pass is stateful for the same reason the Gemini and Chat Completions
//! streaming codecs are: a `tool_use` block is opened frames before the turn
//! ends, and the terminal frame that follows may still report a generic
//! `end_turn`. Only something that remembers the block was opened can correct
//! that, and an uncorrected reason renders as a finished turn -- the client
//! reads it, stops, and never runs the call.
//!
//! A frame maps to more than one event because `message_delta` carries both the
//! terminal `delta.stop_reason` and the final cumulative `usage` — the only
//! place Anthropic reports real token counts for a stream. Returning just one
//! of the two silently discards either the stop signal or the whole cost basis.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — each Anthropic SSE frame is dynamic JSON keyed on
// `type`.
use serde_json::Value;

use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate, ContentBlockKind,
};

/// Per-stream state for the Anthropic SSE pass.
///
/// Carries the message id every terminal frame must echo, and whether the turn
/// opened a tool-use block.
#[derive(Debug, Default)]
pub struct AnthropicStreamState {
    message_id: String,
    saw_tool_use: bool,
}

impl AnthropicStreamState {
    // Why: a decoder that joins a stream already in progress, and every test
    // that drives a single frame, needs the id the terminal frame must echo
    // without replaying a `message_start` to establish it.
    #[must_use]
    pub fn new(message_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            saw_tool_use: false,
        }
    }

    #[must_use]
    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    pub fn events_from_sse(&mut self, value: &Value) -> Vec<CanonicalEvent> {
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            return Vec::new();
        };
        let events = if kind == "message_delta" {
            self.convert_message_delta(value)
        } else {
            single_event(kind, value, &self.message_id)
                .into_iter()
                .collect()
        };
        for event in &events {
            match event {
                CanonicalEvent::MessageStart { id, .. } => self.message_id.clone_from(id),
                CanonicalEvent::ContentBlockStart {
                    block: ContentBlockKind::ToolUse { .. },
                    ..
                } => self.saw_tool_use = true,
                _ => {},
            }
        }
        events
    }

    // Why: Emits the usage before the stop so a consumer that finalizes on the
    // terminal event has already folded in the real token counts.
    fn convert_message_delta(&self, value: &Value) -> Vec<CanonicalEvent> {
        let mut events = Vec::with_capacity(2);
        if let Some(usage) = value.get("usage") {
            let update = usage_update_from_value(usage);
            if !update.is_empty() {
                events.push(CanonicalEvent::UsageDelta(update));
            }
        }
        let stop_reason = value
            .get("delta")
            .and_then(|d| d.get("stop_reason"))
            .and_then(Value::as_str)
            .map(CanonicalStopReason::from_anthropic)
            .map(|r| r.with_tool_use(self.saw_tool_use));
        if stop_reason.is_some() {
            events.push(CanonicalEvent::MessageStop {
                id: self.message_id.clone(),
                stop_reason,
            });
        }
        events
    }
}

fn single_event(kind: &str, value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    match kind {
        "message_start" => convert_message_start(value),
        "content_block_start" => convert_content_block_start(value),
        "content_block_delta" => convert_content_block_delta(value),
        "content_block_stop" => Some(CanonicalEvent::ContentBlockStop {
            index: u32_field(value, "index"),
        }),
        "message_stop" => Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason: None,
        }),
        "error" => Some(CanonicalEvent::Error(
            value
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("upstream error")
                .to_owned(),
        )),
        _ => None,
    }
}

fn convert_message_start(value: &Value) -> Option<CanonicalEvent> {
    let msg = value.get("message")?;
    Some(CanonicalEvent::MessageStart {
        id: str_field(msg, "id", ""),
        model: str_field(msg, "model", ""),
        usage: usage_from_value(msg.get("usage")),
    })
}

fn convert_content_block_start(value: &Value) -> Option<CanonicalEvent> {
    let index = u32_field(value, "index");
    let block = value.get("content_block")?;
    let block_type = block.get("type").and_then(Value::as_str)?;
    let kind = match block_type {
        "text" => ContentBlockKind::Text,
        "thinking" => ContentBlockKind::Thinking {
            id: None,
            signature: block
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
        "tool_use" => ContentBlockKind::ToolUse {
            id: str_field(block, "id", ""),
            name: str_field(block, "name", ""),
            signature: block
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        },
        _ => return None,
    };
    Some(CanonicalEvent::ContentBlockStart { index, block: kind })
}

fn convert_content_block_delta(value: &Value) -> Option<CanonicalEvent> {
    let index = u32_field(value, "index");
    let delta = value.get("delta")?;
    let dtype = delta.get("type").and_then(Value::as_str)?;
    let text_field = |field: &str| str_field(delta, field, "");
    match dtype {
        "text_delta" => Some(CanonicalEvent::TextDelta {
            index,
            text: text_field("text"),
        }),
        "thinking_delta" => Some(CanonicalEvent::ThinkingDelta {
            index,
            text: text_field("thinking"),
        }),
        "signature_delta" => Some(CanonicalEvent::SignatureDelta {
            index,
            signature: text_field("signature"),
        }),
        "input_json_delta" => Some(CanonicalEvent::ToolUseDelta {
            index,
            partial_json: text_field("partial_json"),
        }),
        _ => None,
    }
}

fn usage_update_from_value(u: &Value) -> CanonicalUsageUpdate {
    let field = |name: &str| u.get(name).and_then(Value::as_u64).map(|v| v as u32);
    CanonicalUsageUpdate {
        input_tokens: field("input_tokens"),
        output_tokens: field("output_tokens"),
        cache_read_tokens: field("cache_read_input_tokens"),
        cache_creation_tokens: field("cache_creation_input_tokens"),
        // Why: see `AnthropicUsage::into_canonical` -- Anthropic bills
        // thinking as ordinary output tokens and reports no separate count.
        reasoning_tokens: None,
    }
}

fn usage_from_value(v: Option<&Value>) -> CanonicalUsage {
    let Some(u) = v else {
        return CanonicalUsage::default();
    };
    let field = |name: &str| u.get(name).and_then(Value::as_u64).unwrap_or(0) as u32;
    let input = field("input_tokens");
    let output = field("output_tokens");
    let cache_read = field("cache_read_input_tokens");
    let cache_creation = field("cache_creation_input_tokens");
    CanonicalUsage {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_creation_tokens: cache_creation,
        reasoning_tokens: 0,
        total_tokens: input + output + cache_read + cache_creation,
    }
}

fn str_field(value: &Value, field: &str, default: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn u32_field(value: &Value, field: &str) -> u32 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0) as u32
}
