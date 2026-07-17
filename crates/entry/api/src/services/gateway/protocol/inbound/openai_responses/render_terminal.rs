//! Snapshot-driven terminal frames for the `OpenAI` Responses stream.
//!
//! `response.output_item.done` and `response.completed` carry the
//! authoritative, fully-formed output — a `function_call` item with its
//! complete arguments and the response `output` list. The per-event
//! [`CanonicalEvent`] does not hold that content, so these frames are rendered
//! from the accumulated response snapshot the stream tap maintains. A consumer
//! (e.g. Codex) finalizes and dispatches a tool call from these frames;
//! emitting them incomplete strands the tool call and stops the turn.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — OpenAI Responses wire format is dynamic JSON.
use serde_json::{Value, json};

use super::super::super::canonical::CanonicalContent;
use super::super::super::canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason,
};
use super::render::current_unix_ts;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_terminal_event_frame(
    event: &CanonicalEvent,
    snapshot: &CanonicalResponse,
) -> Option<Bytes> {
    match event {
        CanonicalEvent::ContentBlockStop { index } => render_item_done(*index, snapshot),
        CanonicalEvent::MessageStop { id, stop_reason } => {
            Some(render_completed(id, *stop_reason, snapshot))
        },
        _ => None,
    }
}

fn render_item_done(index: u32, snapshot: &CanonicalResponse) -> Option<Bytes> {
    let block = snapshot.content.get(index as usize)?;
    let item = output_item_value(index, block)?;
    let mut frames = String::new();
    if let CanonicalContent::ToolUse { id, input, .. } = block {
        let arguments = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
        push_frame(
            &mut frames,
            "response.function_call_arguments.done",
            &json!({
                "type": "response.function_call_arguments.done",
                "item_id": format!("fc_{id}"),
                "output_index": index,
                "arguments": arguments,
            }),
        );
    }
    push_frame(
        &mut frames,
        "response.output_item.done",
        &json!({
            "type": "response.output_item.done",
            "output_index": index,
            "item": item,
        }),
    );
    Some(Bytes::from(frames))
}

fn render_completed(
    id: &str,
    stop_reason: Option<CanonicalStopReason>,
    snapshot: &CanonicalResponse,
) -> Bytes {
    let output: Vec<Value> = snapshot
        .content
        .iter()
        .enumerate()
        .filter_map(|(i, block)| output_item_value(i as u32, block))
        .collect();
    let truncated = matches!(stop_reason, Some(CanonicalStopReason::MaxTokens));
    let response = json!({
        "id": id,
        "object": "response",
        "created_at": current_unix_ts(),
        "status": if truncated { "incomplete" } else { "completed" },
        "model": snapshot.model,
        "output": output,
        "usage": {
            "input_tokens": snapshot.usage.input_tokens,
            "output_tokens": snapshot.usage.output_tokens,
            "total_tokens": snapshot.usage.input_tokens + snapshot.usage.output_tokens,
        },
        "incomplete_details": truncated.then(|| json!({ "reason": "max_output_tokens" })),
        "stop_reason": stop_reason.map(CanonicalStopReason::openai_str),
    });
    let event_name = if truncated {
        "response.incomplete"
    } else {
        "response.completed"
    };
    let payload = json!({ "type": event_name, "response": response });
    let mut frame = String::new();
    push_frame(&mut frame, event_name, &payload);
    Bytes::from(frame)
}

fn output_item_value(index: u32, block: &CanonicalContent) -> Option<Value> {
    match block {
        CanonicalContent::Text(text) => Some(json!({
            "type": "message",
            "id": format!("msg_{index}"),
            "status": "completed",
            "role": "assistant",
            "content": [{ "type": "output_text", "text": text, "annotations": [] }],
        })),
        CanonicalContent::ToolUse {
            id, name, input, ..
        } => {
            let arguments = serde_json::to_string(input).unwrap_or_else(|_| "{}".into());
            Some(json!({
                "type": "function_call",
                "id": format!("fc_{id}"),
                "call_id": id,
                "name": name,
                "arguments": arguments,
                "status": "completed",
            }))
        },
        CanonicalContent::Thinking { text, .. } => Some(json!({
            "type": "reasoning",
            "id": format!("rs_{index}"),
            "summary": [{ "type": "summary_text", "text": text }],
        })),
        CanonicalContent::Image(_) | CanonicalContent::ToolResult { .. } => None,
    }
}

fn push_frame(buf: &mut String, event_name: &str, payload: &Value) {
    buf.push_str("event: ");
    buf.push_str(event_name);
    buf.push_str("\ndata: ");
    buf.push_str(&serde_json::to_string(payload).unwrap_or_else(|_| "{}".into()));
    buf.push_str("\n\n");
}
