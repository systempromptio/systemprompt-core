//! `POST /messages` — Anthropic Messages wire-format mock handler.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::{AppState, FIXED_OUTPUT_TOKENS, count_input_tokens, sse};

// Chunked so the streaming path emits multiple `content_block_delta` events.
const REPLY_CHUNKS: &[&str] = &["This ", "is ", "a ", "deterministic ", "mock ", "response."];

fn reply_text() -> String {
    REPLY_CHUNKS.concat()
}

fn new_id() -> String {
    format!("msg_{:016x}", rand::random::<u64>())
}

pub async fn handle(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Response {
    state.apply_latency().await;

    if state.should_fail() {
        return error_response();
    }

    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("mock-anthropic")
        .to_string();
    let input_tokens = count_input_tokens(&body);
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    state.note_request("/messages", &model);

    if streaming {
        let frames = stream_frames(&model, input_tokens);
        sse::response(frames, state.mode)
    } else {
        non_streaming(&model, input_tokens)
    }
}

fn error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "type": "error",
            "error": { "type": "api_error", "message": "mock injected failure" }
        })),
    )
        .into_response()
}

fn non_streaming(model: &str, input_tokens: u32) -> Response {
    Json(json!({
        "id": new_id(),
        "type": "message",
        "role": "assistant",
        "model": model,
        "stop_reason": "end_turn",
        "stop_sequence": Value::Null,
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": FIXED_OUTPUT_TOKENS,
        },
        "content": [
            { "type": "text", "text": reply_text() }
        ],
    }))
    .into_response()
}

fn stream_frames(model: &str, input_tokens: u32) -> Vec<String> {
    let id = new_id();
    let mut frames: Vec<String> = Vec::new();

    frames.push(sse::frame(&json!({
        "type": "message_start",
        "message": {
            "id": id,
            "type": "message",
            "role": "assistant",
            "model": model,
            "content": [],
            "stop_reason": Value::Null,
            "usage": { "input_tokens": input_tokens, "output_tokens": 0 },
        }
    })));
    frames.push(sse::frame(&json!({
        "type": "content_block_start",
        "index": 0,
        "content_block": { "type": "text", "text": "" }
    })));
    for chunk in REPLY_CHUNKS {
        frames.push(sse::frame(&json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": { "type": "text_delta", "text": chunk }
        })));
    }
    frames.push(sse::frame(
        &json!({ "type": "content_block_stop", "index": 0 }),
    ));
    frames.push(sse::frame(&json!({
        "type": "message_delta",
        "delta": { "stop_reason": "end_turn", "stop_sequence": Value::Null },
        "usage": { "input_tokens": input_tokens, "output_tokens": FIXED_OUTPUT_TOKENS }
    })));
    frames.push(sse::frame(&json!({ "type": "message_stop" })));
    frames.push("data: [DONE]\n\n".to_string());
    frames
}
