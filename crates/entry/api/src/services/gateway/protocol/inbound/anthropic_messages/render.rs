//! Renders canonical events as Anthropic Messages SSE frames.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — Anthropic Messages wire format is dynamic JSON.
use serde_json::{Map, Value, json};
use systemprompt_models::wire::anthropic::content_to_anthropic_block;

use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, CanonicalUsageUpdate,
    ContentBlockKind,
};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_response_value(response: &CanonicalResponse) -> Value {
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
        // Why: the streaming render emits all four counts, so a buffered reply
        // that omitted the cache pair reported less usage than the identical
        // streamed one to the same client.
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
            "cache_read_input_tokens": response.usage.cache_read_tokens,
            "cache_creation_input_tokens": response.usage.cache_creation_tokens,
        },
    })
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_event_frame(event: &CanonicalEvent, model: &str) -> Option<Bytes> {
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
        CanonicalEvent::EncryptedContentDelta { .. } => return None,
        CanonicalEvent::SignatureDelta { index, signature } => json!({
            "type": "content_block_delta",
            "index": index,
            "delta": { "type": "signature_delta", "signature": signature },
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
            "usage": render_usage(usage),
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
        .to_owned();
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
        ContentBlockKind::Thinking { signature, .. } => {
            render_thinking_block_start(signature.as_deref())
        },
        ContentBlockKind::ToolUse {
            id,
            name,
            signature,
        } => render_tool_use_block_start(id, name, signature.as_deref()),
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
        obj.insert("signature".into(), Value::String(sig.to_owned()));
    }
    Value::Object(obj)
}

fn render_tool_use_block_start(id: &str, name: &str, signature: Option<&str>) -> Value {
    let mut obj = Map::new();
    obj.insert("type".into(), Value::String("tool_use".into()));
    obj.insert("id".into(), Value::String(id.to_owned()));
    obj.insert("name".into(), Value::String(name.to_owned()));
    obj.insert("input".into(), json!({}));
    if let Some(sig) = signature {
        obj.insert("signature".into(), Value::String(sig.to_owned()));
    }
    Value::Object(obj)
}

fn render_usage(usage: &CanonicalUsageUpdate) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |key: &str, v: Option<u32>| {
        if let Some(v) = v {
            out.insert(key.to_owned(), json!(v));
        }
    };
    put("input_tokens", usage.input_tokens);
    put("output_tokens", usage.output_tokens);
    put("cache_read_input_tokens", usage.cache_read_tokens);
    put("cache_creation_input_tokens", usage.cache_creation_tokens);
    out
}

fn render_message_stop(stop_reason: Option<CanonicalStopReason>) -> Bytes {
    render_message_stop_with_usage(stop_reason, None)
}

// Why: the terminal pair (`message_delta` + `message_stop`) states the counts
// the turn actually used. An Anthropic client reads its output count off
// `message_delta.usage`, and this frame used to state a hardcoded zero -- so a
// streamed turn reported itself as free to every SDK, while the audit row for
// the same request carried the real numbers.
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_terminal_frames(snapshot: &CanonicalResponse) -> Bytes {
    render_message_stop_with_usage(snapshot.stop_reason, Some(&snapshot.usage))
}

fn render_message_stop_with_usage(
    stop_reason: Option<CanonicalStopReason>,
    usage: Option<&CanonicalUsage>,
) -> Bytes {
    let usage_value = usage.map_or_else(
        || json!({ "output_tokens": 0 }),
        |u| {
            json!({
                "input_tokens": u.input_tokens,
                "output_tokens": u.output_tokens,
                "cache_read_input_tokens": u.cache_read_tokens,
                "cache_creation_input_tokens": u.cache_creation_tokens,
            })
        },
    );
    let rendered = json!({
        "type": "message_delta",
        "delta": { "stop_reason": stop_reason.map(CanonicalStopReason::anthropic_str) },
        "usage": usage_value,
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
