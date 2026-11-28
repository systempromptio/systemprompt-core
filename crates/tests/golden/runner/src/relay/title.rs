//! The answer to a session-title call, in the protocol and framing the
//! request asked for.

use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use serde_json::{Value, json};
use systemprompt_models::ApiPaths;

pub(super) fn answer(path: &str, body: &Value, title: &str) -> anyhow::Result<Response> {
    let model = body.get("model").cloned().unwrap_or(Value::Null);
    let stream = body.get("stream").and_then(Value::as_bool) == Some(true);
    let chat = path == ApiPaths::GATEWAY_CHAT_COMPLETIONS;
    let (content_type, text) = match (chat, stream) {
        (false, true) => ("text/event-stream", messages_stream(&model, title)),
        (false, false) => ("application/json", messages_body(&model, title).to_string()),
        (true, true) => ("text/event-stream", chat_stream(&model, title)),
        (true, false) => ("application/json", chat_body(&model, title).to_string()),
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, HeaderValue::from_static(content_type))
        .body(Body::from(text))?)
}

fn messages_body(model: &Value, title: &str) -> Value {
    json!({
        "id": "msg_title", "type": "message", "role": "assistant", "model": model,
        "content": [{ "type": "text", "text": title }],
        "stop_reason": "end_turn", "stop_sequence": null,
        "usage": { "input_tokens": 0, "output_tokens": 0 },
    })
}

fn messages_stream(model: &Value, title: &str) -> String {
    let events = [
        (
            "message_start",
            json!({ "type": "message_start", "message": {
            "id": "msg_title", "type": "message", "role": "assistant", "model": model, "content": [],
            "stop_reason": null, "stop_sequence": null, "usage": { "input_tokens": 0, "output_tokens": 0 } } }),
        ),
        (
            "content_block_start",
            json!({ "type": "content_block_start", "index": 0,
            "content_block": { "type": "text", "text": "" } }),
        ),
        (
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": 0,
            "delta": { "type": "text_delta", "text": title } }),
        ),
        (
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 }),
        ),
        (
            "message_delta",
            json!({ "type": "message_delta",
            "delta": { "stop_reason": "end_turn", "stop_sequence": null }, "usage": { "output_tokens": 0 } }),
        ),
        ("message_stop", json!({ "type": "message_stop" })),
    ];
    events
        .iter()
        .map(|(name, data)| format!("event: {name}\ndata: {data}\n\n"))
        .collect()
}

fn chat_body(model: &Value, title: &str) -> Value {
    json!({
        "id": "chatcmpl-title", "object": "chat.completion", "created": 0, "model": model,
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": title }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
    })
}

fn chat_stream(model: &Value, title: &str) -> String {
    let chunk = |choices: Value, usage: Value| {
        json!({ "id": "chatcmpl-title", "object": "chat.completion.chunk", "created": 0,
            "model": model, "choices": choices, "usage": usage })
    };
    let frames = [
        chunk(
            json!([{ "index": 0, "delta": { "role": "assistant", "content": title }, "finish_reason": null }]),
            Value::Null,
        ),
        chunk(
            json!([{ "index": 0, "delta": {}, "finish_reason": "stop" }]),
            Value::Null,
        ),
        chunk(
            json!([]),
            json!({ "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 }),
        ),
    ];
    let mut text: String = frames
        .iter()
        .map(|frame| format!("data: {frame}\n\n"))
        .collect();
    text.push_str("data: [DONE]\n\n");
    text
}
