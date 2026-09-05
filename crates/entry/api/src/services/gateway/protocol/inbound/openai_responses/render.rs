//! Renders canonical events as `OpenAI` Responses SSE frames.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::{Value, json};

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, ContentBlockKind,
};

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_response_object(response: &CanonicalResponse) -> Value {
    let mut output: Vec<Value> = Vec::new();
    let mut text_parts: Vec<Value> = Vec::new();
    // Why: the Responses object has no finish-reason scalar. A truncated turn
    // is `status: "incomplete"` plus `incomplete_details.reason`, exactly as
    // the streaming terminal frame renders it; anything else is `completed`.
    // The items carry the same verdict -- an item cut mid-arguments is not a
    // completed one, and a client that reads item status must not run it.
    let truncated = matches!(response.stop_reason, Some(CanonicalStopReason::MaxTokens));
    let item_status = if truncated { "incomplete" } else { "completed" };

    for part in &response.content {
        match part {
            CanonicalContent::Text(t) => {
                text_parts.push(json!({ "type": "output_text", "text": t, "annotations": [] }));
            },
            CanonicalContent::ToolUse {
                id, name, input, ..
            } => {
                let arguments = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
                output.push(json!({
                    "type": "function_call",
                    "id": format!("fc_{id}"),
                    "call_id": id,
                    "name": name,
                    "arguments": arguments,
                    "status": item_status,
                }));
            },
            CanonicalContent::Thinking {
                text,
                id,
                encrypted_content,
                ..
            } => output.push(reasoning_output_item(
                id.as_deref(),
                &format!("rs_{}", response.id),
                text,
                encrypted_content.as_deref(),
            )),
            CanonicalContent::Image(_) | CanonicalContent::ToolResult { .. } => {},
        }
    }

    if !text_parts.is_empty() {
        output.insert(
            0,
            json!({
                "type": "message",
                "id": format!("msg_{}", response.id),
                "status": item_status,
                "role": "assistant",
                "content": text_parts,
            }),
        );
    }

    json!({
        "id": response.id,
        "object": "response",
        "created_at": current_unix_ts(),
        "status": if truncated { "incomplete" } else { "completed" },
        "model": response.model,
        "output": output,
        "usage": {
            "input_tokens": response.usage.input_tokens,
            "output_tokens": response.usage.output_tokens,
            "total_tokens": response.usage.input_tokens + response.usage.output_tokens,
        },
        "incomplete_details": truncated.then(|| json!({ "reason": "max_output_tokens" })),
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
    let (event_name, payload): (&str, Value) = match event {
        CanonicalEvent::MessageStart {
            id,
            model: m,
            usage,
        } => (
            "response.created",
            json!({
                "type": "response.created",
                "response": {
                    "id": id,
                    "object": "response",
                    "created_at": current_unix_ts(),
                    "status": "in_progress",
                    "model": if m.is_empty() { model } else { m },
                    "output": [],
                    "usage": {
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                    },
                },
            }),
        ),
        CanonicalEvent::ContentBlockStart { index, block } => (
            "response.output_item.added",
            render_block_start(*index, block),
        ),
        CanonicalEvent::TextDelta { index, text } => (
            "response.output_text.delta",
            json!({
                "type": "response.output_text.delta",
                "output_index": index,
                "content_index": 0,
                "delta": text,
            }),
        ),
        CanonicalEvent::ThinkingDelta { index, text } => (
            "response.reasoning_summary_text.delta",
            json!({
                "type": "response.reasoning_summary_text.delta",
                "output_index": index,
                "summary_index": 0,
                "delta": text,
            }),
        ),
        CanonicalEvent::ToolUseDelta {
            index,
            partial_json,
        } => (
            "response.function_call_arguments.delta",
            json!({
                "type": "response.function_call_arguments.delta",
                "output_index": index,
                "delta": partial_json,
            }),
        ),
        CanonicalEvent::ContentBlockStop { .. }
        | CanonicalEvent::MessageStop { .. }
        | CanonicalEvent::UsageDelta(_)
        | CanonicalEvent::SignatureDelta { .. }
        | CanonicalEvent::EncryptedContentDelta { .. } => return None,
        CanonicalEvent::Error(msg) => return Some(render_error_frame(msg)),
    };
    Some(Bytes::from(format!(
        "event: {event_name}\ndata: {}\n\n",
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
    )))
}

fn render_block_start(index: u32, block: &ContentBlockKind) -> Value {
    match block {
        ContentBlockKind::Text => json!({
            "type": "response.output_item.added",
            "output_index": index,
            "item": {
                "type": "message",
                "id": format!("msg_{index}"),
                "status": "in_progress",
                "role": "assistant",
                "content": [],
            },
        }),
        ContentBlockKind::ToolUse { id, name, .. } => json!({
            "type": "response.output_item.added",
            "output_index": index,
            "item": {
                "type": "function_call",
                "id": format!("fc_{id}"),
                "call_id": id,
                "name": name,
                "arguments": "",
                "status": "in_progress",
            },
        }),
        ContentBlockKind::Thinking { id, .. } => json!({
            "type": "response.output_item.added",
            "output_index": index,
            "item": {
                "type": "reasoning",
                "id": id.clone().unwrap_or_else(|| format!("rs_{index}")),
                "summary": [],
            },
        }),
    }
}

pub(super) fn reasoning_output_item(
    id: Option<&str>,
    fallback_id: &str,
    text: &str,
    encrypted_content: Option<&str>,
) -> Value {
    let mut item = serde_json::Map::new();
    item.insert("type".into(), json!("reasoning"));
    item.insert("id".into(), json!(id.unwrap_or(fallback_id)));
    item.insert(
        "summary".into(),
        if text.is_empty() {
            json!([])
        } else {
            json!([{ "type": "summary_text", "text": text }])
        },
    );
    if let Some(enc) = encrypted_content {
        item.insert("encrypted_content".into(), json!(enc));
    }
    Value::Object(item)
}

fn render_error_frame(msg: &str) -> Bytes {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    // Why: the Responses contract puts the outcome on `response.status`, and a
    // client (Codex) reads that field to decide the turn failed; a bare
    // top-level `error` left the response object absent and the turn unresolved.
    Bytes::from(format!(
        "event: response.failed\ndata: \
         {{\"type\":\"response.failed\",\"response\":{{\"status\":\"failed\",\"error\":\
         {{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}},\"error\":\
         {{\"type\":\"api_error\",\"message\":\"{escaped}\"}}}}\n\n"
    ))
}
