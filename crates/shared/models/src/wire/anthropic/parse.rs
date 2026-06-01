//! Anthropic Messages response + SSE-frame parse side of the codec.
//!
//! [`parse_response`] turns a buffered Messages reply into a
//! [`CanonicalResponse`]; [`event_from_sse`] turns one decoded SSE `data:`
//! payload into a [`CanonicalEvent`]. Both are pure functions over
//! [`serde_json::Value`].

// JSON: protocol boundary — the Anthropic Messages wire format is dynamic JSON.
use serde_json::Value;

use crate::wire::canonical::{
    CanonicalContent, CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage,
    ContentBlockKind, ImageSource,
};

#[must_use]
pub fn parse_response(value: &Value, fallback_model: &str) -> CanonicalResponse {
    let id = str_field(value, "id", "");
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(fallback_model)
        .to_owned();
    let stop_reason = value
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(CanonicalStopReason::from_anthropic);
    let usage = usage_from_value(value.get("usage"));

    let mut content: Vec<CanonicalContent> = Vec::new();
    if let Some(arr) = value.get("content").and_then(Value::as_array) {
        for block in arr {
            if let Some(part) = parse_content_block(block) {
                content.push(part);
            }
        }
    }

    CanonicalResponse {
        id,
        model,
        content,
        stop_reason,
        usage,
    }
}

fn parse_content_block(value: &Value) -> Option<CanonicalContent> {
    let kind = value.get("type").and_then(Value::as_str)?;
    match kind {
        "text" => Some(CanonicalContent::Text(str_field(value, "text", ""))),
        "thinking" => Some(CanonicalContent::Thinking {
            text: str_field(value, "thinking", ""),
            signature: value
                .get("signature")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        "tool_use" => Some(CanonicalContent::ToolUse {
            id: str_field(value, "id", ""),
            name: str_field(value, "name", ""),
            input: value.get("input").cloned().unwrap_or(Value::Null),
        }),
        "image" => parse_image_block(value),
        _ => None,
    }
}

fn parse_image_block(value: &Value) -> Option<CanonicalContent> {
    let src = value.get("source")?;
    let stype = src.get("type").and_then(Value::as_str)?;
    match stype {
        "base64" => Some(CanonicalContent::Image(ImageSource::Base64 {
            media_type: src
                .get("media_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_owned(),
            data: str_field(src, "data", ""),
        })),
        "url" => Some(CanonicalContent::Image(ImageSource::Url(str_field(
            src, "url", "",
        )))),
        _ => None,
    }
}

/// Translate one decoded SSE `data:` JSON payload into a canonical event.
///
/// `msg_id` carries the message id observed at `message_start` so later
/// `message_stop` frames can be tagged. Returns `None` for frames the canonical
/// model does not model (e.g. `ping`).
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
    CanonicalUsage {
        input_tokens: u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
        output_tokens: u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as u32,
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
