//! Renders canonical responses and events as `OpenAI` Chat Completions wire
//! frames.
//!
//! Streaming chunks use the bare `data: {json}` SSE dialect (no `event:`
//! lines) and terminate with `data: [DONE]`, as the Chat Completions API does.
//! Chunk ids use a fixed synthetic value: the upstream message id is only
//! known at stream start and end, and chat clients aggregate chunks by
//! `choices[].delta`, never by id — a constant keeps every chunk consistent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};

pub(super) const STREAM_CHUNK_ID: &str = "chatcmpl-systemprompt-stream";

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_response_object(response: &CanonicalResponse) -> Value {
    let mut text = String::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    for part in &response.content {
        match part {
            CanonicalContent::Text(t) => text.push_str(t),
            CanonicalContent::ToolUse {
                id, name, input, ..
            } => {
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(input)
                            .unwrap_or_else(|_| "{}".into()),
                    },
                }));
            },
            CanonicalContent::Thinking { .. }
            | CanonicalContent::Image(_)
            | CanonicalContent::ToolResult { .. } => {},
        }
    }

    let mut message = Map::new();
    message.insert("role".into(), Value::String("assistant".into()));
    message.insert(
        "content".into(),
        if text.is_empty() {
            Value::Null
        } else {
            Value::String(text)
        },
    );
    if !tool_calls.is_empty() {
        message.insert("tool_calls".into(), Value::Array(tool_calls));
    }

    json!({
        "id": response.id,
        "object": "chat.completion",
        "created": current_unix_ts(),
        "model": response.model,
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": finish_reason(response.stop_reason),
        }],
        "usage": usage_object(&response.usage),
    })
}

pub(super) fn finish_reason(stop_reason: Option<CanonicalStopReason>) -> &'static str {
    stop_reason.map_or("stop", CanonicalStopReason::openai_str)
}

pub(super) fn usage_object(usage: &CanonicalUsage) -> Value {
    json!({
        "prompt_tokens": usage.input_tokens,
        "completion_tokens": usage.output_tokens,
        "total_tokens": usage.input_tokens + usage.output_tokens,
        "prompt_tokens_details": { "cached_tokens": usage.cache_read_tokens },
    })
}

pub(super) fn current_unix_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_event_frame(event: &CanonicalEvent, model: &str) -> Option<Bytes> {
    let delta: Value = match event {
        CanonicalEvent::MessageStart { .. } => json!({ "role": "assistant", "content": "" }),
        CanonicalEvent::TextDelta { text, .. } => json!({ "content": text }),
        CanonicalEvent::ContentBlockStart { index, block } => match block {
            ContentBlockKind::ToolUse { id, name, .. } => json!({
                "tool_calls": [{
                    "index": index,
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": "" },
                }],
            }),
            ContentBlockKind::Text | ContentBlockKind::Thinking { .. } => return None,
        },
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => json!({
            "tool_calls": [{
                "index": index,
                "function": { "arguments": partial_json },
            }],
        }),
        CanonicalEvent::Error(msg) => return Some(render_error_frame(msg)),
        CanonicalEvent::ThinkingDelta { .. }
        | CanonicalEvent::SignatureDelta { .. }
        | CanonicalEvent::EncryptedContentDelta { .. }
        | CanonicalEvent::ContentBlockStop { .. }
        | CanonicalEvent::UsageDelta(_)
        | CanonicalEvent::MessageStop { .. } => return None,
    };
    Some(render_chunk(model, delta, None, None))
}

pub(super) fn render_chunk(
    model: &str,
    delta: Value,
    finish: Option<&str>,
    usage: Option<Value>,
) -> Bytes {
    let mut chunk = Map::new();
    chunk.insert("id".into(), Value::String(STREAM_CHUNK_ID.into()));
    chunk.insert(
        "object".into(),
        Value::String("chat.completion.chunk".into()),
    );
    chunk.insert("created".into(), Value::from(current_unix_ts()));
    chunk.insert("model".into(), Value::String(model.to_owned()));
    chunk.insert(
        "choices".into(),
        json!([{
            "index": 0,
            "delta": delta,
            "finish_reason": finish,
        }]),
    );
    if let Some(u) = usage {
        chunk.insert("usage".into(), u);
    }
    let body = serde_json::to_string(&Value::Object(chunk)).unwrap_or_else(|_| "{}".into());
    Bytes::from(format!("data: {body}\n\n"))
}

fn render_error_frame(msg: &str) -> Bytes {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    Bytes::from(format!(
        "data: {{\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
    ))
}
