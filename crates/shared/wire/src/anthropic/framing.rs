//! Anthropic SSE byte framing: raw upstream bytes to canonical events.
//!
//! [`AnthropicStreamState`] maps one decoded `data:` payload; this module owns
//! the step before it — framing bytes through the shared
//! [`SseFramer`](crate::sse::SseFramer) and decoding each payload. The gateway
//! and the in-process AI service both read Anthropic streams (first-party and
//! Vertex AI send the same frames), so the framing lives once, here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
// JSON: protocol boundary — each SSE payload is dynamic JSON keyed on `type`.
use serde_json::Value;

use super::sse::AnthropicStreamState;
use crate::canonical::CanonicalEvent;
use crate::error::WireStreamError;
use crate::sse::{SseFramer, skip_frame, skip_undecodable_json};

const WIRE: &str = "anthropic";

#[derive(Debug, Default)]
pub struct SseFrameDecoder {
    framer: SseFramer,
    codec: AnthropicStreamState,
}

impl SseFrameDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<CanonicalEvent, WireStreamError>> {
        let mut events = Vec::new();
        for frame in self.framer.push(chunk) {
            let frame = match frame {
                Ok(frame) => frame,
                Err(e) => {
                    events.push(Err(e.into()));
                    continue;
                },
            };
            let data = frame.data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            match serde_json::from_str::<Value>(data) {
                Ok(value) if value.get("type").is_some_and(Value::is_string) => {
                    events.extend(self.codec.events_from_sse(&value).into_iter().map(Ok));
                },
                Ok(_) => skip_frame(WIRE, "frame has no string `type`", data.len()),
                Err(e) => skip_undecodable_json(WIRE, &e, data.len()),
            }
        }
        events
    }
}

pub fn sse_to_canonical_events<S, E>(
    stream: S,
) -> BoxStream<'static, Result<CanonicalEvent, WireStreamError>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>> + 'static,
{
    stream
        .scan(SseFrameDecoder::default(), |decoder, item| {
            let out: Vec<Result<CanonicalEvent, WireStreamError>> = match item {
                Ok(bytes) => decoder.push(&bytes),
                Err(e) => vec![Err(WireStreamError::transport(e))],
            };
            futures_util::future::ready(Some(out))
        })
        .flat_map(stream::iter)
        .boxed()
}
