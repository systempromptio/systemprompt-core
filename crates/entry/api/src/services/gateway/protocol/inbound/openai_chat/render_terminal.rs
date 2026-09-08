//! Snapshot-driven terminal frames for the Chat Completions stream.
//!
//! The final chunk carries the authoritative `finish_reason` and the usage
//! block — both come from the accumulated response snapshot the stream tap
//! maintains, because the per-event [`CanonicalEvent`] does not hold complete
//! usage. The stream closes with the `data: [DONE]` sentinel every Chat
//! Completions client waits for; without it `OpenCode` and Copilot treat the
//! turn as aborted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
// JSON: protocol boundary — OpenAI Chat Completions wire format is dynamic
// JSON.
use serde_json::json;

use super::super::super::canonical_response::{CanonicalEvent, CanonicalResponse};
use super::render::{finish_reason, render_chunk, usage_object};

pub fn render_terminal_event_frame(
    event: &CanonicalEvent,
    snapshot: &CanonicalResponse,
) -> Option<Bytes> {
    let CanonicalEvent::MessageStop { stop_reason, .. } = event else {
        return None;
    };
    let reason = finish_reason(stop_reason.or(snapshot.stop_reason));
    // Why: Chat Completions can send usage after the finish chunk.
    Some(render_chunk(
        &snapshot.model,
        &json!({}),
        Some(reason),
        None,
    ))
}

pub(super) fn render_stream_tail_frames(
    snapshot: &CanonicalResponse,
    include_usage: bool,
) -> Bytes {
    let mut frames: Vec<u8> = Vec::new();
    if include_usage {
        frames.extend_from_slice(&render_usage_chunk(snapshot));
    }
    frames.extend_from_slice(b"data: [DONE]\n\n");
    Bytes::from(frames)
}

fn render_usage_chunk(snapshot: &CanonicalResponse) -> Bytes {
    let payload = json!({
        "id": snapshot.id,
        "object": "chat.completion.chunk",
        "created": super::render::current_unix_ts(),
        "model": snapshot.model,
        "choices": [],
        "usage": usage_object(&snapshot.usage),
    });
    Bytes::from(format!(
        "data: {}\n\n",
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
    ))
}
