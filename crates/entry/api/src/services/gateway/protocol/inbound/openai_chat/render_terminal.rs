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
    let CanonicalEvent::MessageStop { stop_reason, .. } = event else {
        return None;
    };
    let reason = finish_reason(stop_reason.or(snapshot.stop_reason));
    // Why: the counts are not known yet. Chat Completions sends usage in a
    // chunk of its own after the finish chunk, so anything rendered here is a
    // zero -- and a zeroed `usage` object is worse than none, because a client
    // that asked for usage reads it and reports the turn as free. The counts
    // and the sentinel are rendered by `render_stream_tail_frames`, once the
    // stream has actually ended.
    Some(render_chunk(
        &snapshot.model,
        &json!({}),
        Some(reason),
        None,
    ))
}

/// The frames that close the stream: the contract's usage-only chunk, then the
/// `[DONE]` sentinel.
///
/// `include_usage` is the caller's own `stream_options.include_usage`. The
/// contract is explicit that usage is reported in a trailing chunk whose
/// `choices` array is empty, and only when it was asked for.
#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn render_stream_tail_frames(snapshot: &CanonicalResponse, include_usage: bool) -> Bytes {
    let mut frames: Vec<u8> = Vec::new();
    if include_usage {
        frames.extend_from_slice(&render_usage_chunk(snapshot));
    }
    frames.extend_from_slice(b"data: [DONE]\n\n");
    Bytes::from(frames)
}

// Why: the usage chunk carries an empty `choices` array, which is how an
// OpenAI-SDK client tells it apart from a content chunk and stops looking for
// a delta on it.
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
