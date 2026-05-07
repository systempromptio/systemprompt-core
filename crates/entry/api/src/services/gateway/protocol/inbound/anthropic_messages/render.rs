use bytes::Bytes;
// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::{CanonicalContent, ImageSource};
use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

pub(super) fn render_response_value(response: &CanonicalResponse) -> Value {
    let content: Vec<Value> = response
        .content
        .iter()
        .map(content_to_anthropic_block)
        .collect();
    json!({
        "id": response.id,
        "type": "message",
        "role": "assistant",
        "model": response.model,
        "content": content,
        "stop_reason": response.stop_reason.map(CanonicalStopReason::anthropic_str),
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
        },
    })
}

pub fn content_to_anthropic_block(part: &CanonicalContent) -> Value {
    match part {
        CanonicalContent::Text(t) => json!({ "type": "text", "text": t }),
        CanonicalContent::Thinking { text, signature } => {
            let mut obj = Map::new();
            obj.insert("type".into(), Value::String("thinking".into()));
            obj.insert("thinking".into(), Value::String(text.clone()));
            if let Some(sig) = signature {
                obj.insert("signature".into(), Value::String(sig.clone()));
            }
            Value::Object(obj)
        },
        CanonicalContent::ToolUse { id, name, input } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }),
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let inner: Vec<Value> = content.iter().map(content_to_anthropic_block).collect();
            json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "is_error": is_error,
                "content": inner,
            })
        },
        CanonicalContent::Image(src) => match src {
            ImageSource::Base64 { media_type, data } => json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": media_type,
                    "data": data,
                },
            }),
            ImageSource::Url(u) => json!({
                "type": "image",
                "source": { "type": "url", "url": u },
            }),
        },
    }
}

#[allow(clippy::unnecessary_wraps)]
pub(super) fn render_event_frame(event: &CanonicalEvent, model: &str) -> Option<Bytes> {
    let value = match event {
        CanonicalEvent::MessageStart {
            id,
            model: m,
            usage,
        } => render_message_start(id, m, model, usage),
        CanonicalEvent::ContentBlockStart { index, block } => {
            render_content_block_start(*index, block)
        },
        CanonicalEvent::TextDelta { index, text } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "text_delta", "text": text },
        }),
        CanonicalEvent::ThinkingDelta { index, text } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "thinking_delta", "thinking": text },
        }),
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "input_json_delta", "partial_json": partial_json },
        }),
        CanonicalEvent::ContentBlockStop { index } => json!({
            "type": "content_block_stop",
            "index": index,
        }),
        CanonicalEvent::UsageDelta(usage) => json!({
            "type": "message_delta",
            "delta": {},
            "usage": {
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
            },
        }),
        CanonicalEvent::MessageStop { stop_reason, .. } => {
            return Some(render_message_stop(*stop_reason));
        },
        CanonicalEvent::Error(msg) => return Some(render_error_frame(msg)),
    };
    let event_name = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message")
        .to_string();
    Some(Bytes::from(format!(
        "event: {event_name}\ndata: {}\n\n",
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".into())
    )))
}

fn render_message_start(
    id: &str,
    event_model: &str,
    fallback_model: &str,
    usage: &CanonicalUsage,
) -> Value {
    json!({
        "type": "message_start",
        "message": {
            "id": id,
            "type": "message",
            "role": "assistant",
            "model": if event_model.is_empty() { fallback_model } else { event_model },
            "content": [],
            "stop_reason": Value::Null,
            "stop_sequence": Value::Null,
            "usage": {
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
            },
        },
    })
}

fn render_content_block_start(index: u32, block: &ContentBlockKind) -> Value {
    let block_value = match block {
        ContentBlockKind::Text => json!({ "type": "text", "text": "" }),
        ContentBlockKind::Thinking { signature } => {
            render_thinking_block_start(signature.as_deref())
        },
        ContentBlockKind::ToolUse { id, name } => json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": {},
        }),
    };
    json!({
        "type": "content_block_start",
        "index": index,
        "content_block": block_value,
    })
}

fn render_thinking_block_start(signature: Option<&str>) -> Value {
    let mut obj = Map::new();
    obj.insert("type".into(), Value::String("thinking".into()));
    obj.insert("thinking".into(), Value::String(String::new()));
    if let Some(sig) = signature {
        obj.insert("signature".into(), Value::String(sig.to_string()));
    }
    Value::Object(obj)
}

fn render_message_stop(stop_reason: Option<CanonicalStopReason>) -> Bytes {
    let rendered = json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason.map(CanonicalStopReason::anthropic_str) },
        "usage": { "output_tokens": 0 },
    });
    Bytes::from(format!(
        "event: message_delta\ndata: {}\n\nevent: message_stop\ndata: \
         {{\"type\":\"message_stop\"}}\n\n",
        serde_json::to_string(&rendered).unwrap_or_else(|_| "{}".into())
    ))
}

fn render_error_frame(msg: &str) -> Bytes {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    Bytes::from(format!(
        "event: error\ndata: \
         {{\"type\":\"error\",\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
    ))
}
