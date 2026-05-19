//! `POST /chat/completions` — OpenAI Chat Completions wire-format mock handler.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::{AppState, FIXED_OUTPUT_TOKENS, count_input_tokens, sse};

// Chunked so the streaming path emits multiple delta chunks.
const REPLY_CHUNKS: &[&str] = &["This ", "is ", "a ", "deterministic ", "mock ", "response."];

fn reply_text() -> String {
    REPLY_CHUNKS.concat()
}

fn new_id() -> String {
    format!("chatcmpl-{:016x}", rand::random::<u64>())
}

pub async fn handle(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Response {
    state.apply_latency().await;

    if state.should_fail() {
        return error_response();
    }

    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("mock-openai")
        .to_string();
    let input_tokens = count_input_tokens(&body);
    let streaming = body.get("stream").and_then(Value::as_bool).unwrap_or(false);
    state.note_request("/chat/completions", &model);

    if streaming {
        let include_usage = body
            .get("stream_options")
            .and_then(|o| o.get("include_usage"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let frames = stream_frames(&model, input_tokens, include_usage);
        sse::response(frames, state.mode)
    } else {
        non_streaming(&model, input_tokens)
    }
}

fn error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": {
                "type": "api_error",
                "message": "mock injected failure",
                "code": Value::Null,
            }
        })),
    )
        .into_response()
}

fn non_streaming(model: &str, input_tokens: u32) -> Response {
    Json(json!({
        "id": new_id(),
        "object": "chat.completion",
        "model": model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": reply_text() },
            "finish_reason": "stop",
        }],
        "usage": {
            "prompt_tokens": input_tokens,
            "completion_tokens": FIXED_OUTPUT_TOKENS,
            "total_tokens": input_tokens + FIXED_OUTPUT_TOKENS,
        },
    }))
    .into_response()
}

fn stream_frames(model: &str, input_tokens: u32, include_usage: bool) -> Vec<String> {
    let id = new_id();
    let mut frames: Vec<String> = Vec::new();

    for chunk in REPLY_CHUNKS {
        frames.push(sse::frame(&json!({
            "id": id,
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [{
                "index": 0,
                "delta": { "content": chunk },
                "finish_reason": Value::Null,
            }],
        })));
    }
    frames.push(sse::frame(&json!({
        "id": id,
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop",
        }],
    })));
    if include_usage {
        frames.push(sse::frame(&json!({
            "id": id,
            "object": "chat.completion.chunk",
            "model": model,
            "choices": [],
            "usage": {
                "prompt_tokens": input_tokens,
                "completion_tokens": FIXED_OUTPUT_TOKENS,
                "total_tokens": input_tokens + FIXED_OUTPUT_TOKENS,
            },
        })));
    }
    frames.push("data: [DONE]\n\n".to_string());
    frames
}
