//! Anthropic Messages streaming-frame parse side of the codec.
//!
//! [`event_from_sse`] turns one decoded SSE `data:` payload into a
//! [`CanonicalEvent`]. The streaming side stays dynamic because each frame is a
//! distinct, sparsely-populated event keyed on `type`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — each Anthropic SSE frame is dynamic JSON keyed on
// `type`.
use serde_json::Value;

use crate::wire::canonical::{
    CanonicalEvent, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

#[must_use]
pub fn event_from_sse(value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "message_start" => convert_message_start(value),
        "content_block_start" => convert_content_block_start(value),
        "content_block_delta" => convert_content_block_delta(value),
        "content_block_stop" => Some(CanonicalEvent::ContentBlockStop {
            index: u32_field(value, "index"),
        }),
        "message_delta" => convert_message_delta(value, msg_id),
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

fn convert_message_delta(value: &Value, msg_id: &str) -> Option<CanonicalEvent> {
    let stop_reason = value
        .get("delta")
        .and_then(|d| d.get("stop_reason"))
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_anthropic);
    if stop_reason.is_some() {
        return Some(CanonicalEvent::MessageStop {
            id: msg_id.to_owned(),
            stop_reason,
        });
    }
    value
        .get("usage")
        .map(|u| CanonicalEvent::UsageDelta(usage_from_value(Some(u))))
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
