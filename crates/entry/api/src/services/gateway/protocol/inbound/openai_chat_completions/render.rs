use bytes::Bytes;
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic JSON.
use serde_json::{Map, Value, json};

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, ContentBlockKind,
};

// Streaming chunks carry no per-response id at delta time (the trait renders one
// event at a time with no shared state), so every chunk in a stream shares this
// stable id — clients group by it within a single response, which is all the
// Chat Completions contract requires.
const STREAM_ID: &str = "chatcmpl-stream";

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
            CanonicalContent::ToolUse { id, name, input } => {
                let arguments = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
                tool_calls.push(json!({
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": arguments },
                }));
            },
            CanonicalContent::Thinking { .. }
            | CanonicalContent::Image(_)
            | CanonicalContent::ToolResult { .. } => {},
        }
    }

    let mut message = Map::new();
    message.insert("role".into(), json!("assistant"));
    let content = if text.is_empty() && !tool_calls.is_empty() {
        Value::Null
    } else {
        json!(text)
    };
    message.insert("content".into(), content);
    if !tool_calls.is_empty() {
        message.insert("tool_calls".into(), Value::Array(tool_calls));
    }

    json!({
        "id": format!("chatcmpl-{}", response.id),
        "object": "chat.completion",
        "created": current_unix_ts(),
        "model": response.model,
        "choices": [{
            "index": 0,
            "message": Value::Object(message),
            "finish_reason": response.stop_reason.map(CanonicalStopReason::openai_str),
        }],
        "usage": {
            "prompt_tokens": response.usage.input_tokens,
            "completion_tokens": response.usage.output_tokens,
            "total_tokens": response.usage.input_tokens + response.usage.output_tokens,
        },
    })
}

fn current_unix_ts() -> u64 {
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
    match event {
        CanonicalEvent::MessageStart { model: m, .. } => {
            let model = if m.is_empty() { model } else { m };
            Some(frame(&chunk(model, &json!({ "role": "assistant" }), None)))
        },
        CanonicalEvent::ContentBlockStart { index, block } => render_block_start(*index, block, model),
        CanonicalEvent::TextDelta { text, .. } => {
            Some(frame(&chunk(model, &json!({ "content": text }), None)))
        },
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => {
            let delta = json!({
                "tool_calls": [{ "index": index, "function": { "arguments": partial_json } }],
            });
            Some(frame(&chunk(model, &delta, None)))
        },
        CanonicalEvent::MessageStop { stop_reason, .. } => {
            let finish = stop_reason.map_or("stop", CanonicalStopReason::openai_str);
            let mut out = frame(&chunk(model, &json!({}), Some(finish))).to_vec();
            out.extend_from_slice(b"data: [DONE]\n\n");
            Some(Bytes::from(out))
        },
        CanonicalEvent::Error(msg) => Some(render_error_frame(msg)),
        CanonicalEvent::ThinkingDelta { .. }
        | CanonicalEvent::ContentBlockStop { .. }
        | CanonicalEvent::UsageDelta(_) => None,
    }
}

fn render_block_start(index: u32, block: &ContentBlockKind, model: &str) -> Option<Bytes> {
    match block {
        ContentBlockKind::ToolUse { id, name } => {
            let delta = json!({
                "tool_calls": [{
                    "index": index,
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": "" },
                }],
            });
            Some(frame(&chunk(model, &delta, None)))
        },
        ContentBlockKind::Text | ContentBlockKind::Thinking { .. } => None,
    }
}

fn chunk(model: &str, delta: &Value, finish_reason: Option<&str>) -> Value {
    json!({
        "id": STREAM_ID,
        "object": "chat.completion.chunk",
        "created": current_unix_ts(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": delta,
            "finish_reason": finish_reason,
        }],
    })
}

fn frame(value: &Value) -> Bytes {
    Bytes::from(format!(
        "data: {}\n\n",
        serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
    ))
}

fn render_error_frame(msg: &str) -> Bytes {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    Bytes::from(format!(
        "data: {{\"error\":{{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
    ))
}
