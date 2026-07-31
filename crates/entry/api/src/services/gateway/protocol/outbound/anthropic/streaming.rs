//! Decodes Anthropic SSE frames into canonical events.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::StreamExt;
// JSON: protocol boundary — event shapes are owned by the models::wire
// Anthropic codec.
use serde_json::Value;
use systemprompt_models::wire::anthropic;

use super::super::super::canonical_response::CanonicalEvent;

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn sse_to_canonical_events<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    use futures_util::stream;
    let s = stream
        .map(|chunk| chunk.map_err(|e| e.to_string()))
        .scan((Vec::<u8>::new(), String::new()), |state, item| {
            let (buf, msg_id) = state;
            let res = match item {
                Ok(bytes) => {
                    buf.extend_from_slice(&bytes);
                    Some(drain_frames(buf, msg_id))
                },
                Err(e) => Some(vec![Err(e)]),
            };
            futures_util::future::ready(res)
        })
        .flat_map(stream::iter);
    s.boxed()
}

/// Relays the upstream SSE byte stream unchanged.
///
/// Used when the caller and the upstream speak the same wire protocol: decoding
/// to [`CanonicalEvent`] and re-rendering would drop any event type or field
/// the canonical model does not describe, and the client is entitled to the
/// frames the provider actually sent. Usage accounting reads a copy downstream.
pub(in crate::services::gateway) fn raw_sse_stream<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<bytes::Bytes, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    stream.map(|chunk| chunk.map_err(|e| e.to_string())).boxed()
}

/// Incremental Anthropic SSE decoder for callers that also need the raw bytes.
///
/// The byte-passthrough lane still has to account for usage and record a
/// response snapshot, so it feeds a copy of every chunk through this decoder
/// while the untouched chunk continues to the client.
#[derive(Debug, Default)]
pub(in crate::services::gateway) struct SseDecoder {
    buf: Vec<u8>,
    msg_id: String,
}

impl SseDecoder {
    pub(in crate::services::gateway) fn push(&mut self, chunk: &[u8]) -> Vec<CanonicalEvent> {
        self.buf.extend_from_slice(chunk);
        drain_frames(&mut self.buf, &mut self.msg_id)
            .into_iter()
            .flatten()
            .collect()
    }
}

fn drain_frames(buf: &mut Vec<u8>, msg_id: &mut String) -> Vec<Result<CanonicalEvent, String>> {
    let mut events: Vec<Result<CanonicalEvent, String>> = Vec::new();
    while let Some(end) = systemprompt_models::wire::sse::frame_end(buf) {
        let frame: Vec<u8> = buf.drain(..end).collect();
        let frame_str = String::from_utf8_lossy(&frame);
        for line in frame_str.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(data)
                    && let Some(ev) = anthropic::event_from_sse(&value, msg_id)
                {
                    if let CanonicalEvent::MessageStart { id, .. } = &ev {
                        msg_id.clone_from(id);
                    }
                    events.push(Ok(ev));
                }
            }
        }
    }
    events
}
