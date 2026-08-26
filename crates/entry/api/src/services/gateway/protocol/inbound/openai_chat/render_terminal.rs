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
    let final_chunk = render_chunk(
        &snapshot.model,
        &json!({}),
        Some(reason),
        Some(usage_object(&snapshot.usage)),
    );
    let mut frames = Vec::with_capacity(final_chunk.len() + 15);
    frames.extend_from_slice(&final_chunk);
    frames.extend_from_slice(b"data: [DONE]\n\n");
    Some(Bytes::from(frames))
}
