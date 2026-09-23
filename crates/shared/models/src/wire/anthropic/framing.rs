//! Anthropic SSE byte framing: raw upstream bytes to canonical events.
//!
//! [`AnthropicStreamState`] maps one decoded `data:` payload; this module owns
//! the step before it — buffering bytes until a frame is complete, splitting
//! it into lines and decoding each payload. The gateway and the in-process AI
//! service both read Anthropic streams (first-party and Vertex AI send the
//! same frames), so the framing lives once, here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::stream::{self, BoxStream, Stream, StreamExt};
// JSON: protocol boundary — each SSE payload is dynamic JSON keyed on `type`.
use serde_json::Value;

use super::sse::AnthropicStreamState;
use crate::wire::canonical::CanonicalEvent;

#[derive(Debug, Default)]
pub struct SseFrameDecoder {
    buf: Vec<u8>,
    codec: AnthropicStreamState,
}

impl SseFrameDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<CanonicalEvent> {
        self.buf.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(end) = crate::wire::sse::frame_end(&self.buf) {
            let frame: Vec<u8> = self.buf.drain(..end).collect();
            let frame_str = String::from_utf8_lossy(&frame);
            for line in frame_str.lines() {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(value) = serde_json::from_str::<Value>(data) {
                    events.extend(self.codec.events_from_sse(&value));
                }
            }
        }
        events
    }
}

pub fn sse_to_canonical_events<S, E>(
    stream: S,
) -> BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: Stream<Item = Result<Bytes, E>> + Send + 'static,
    E: std::fmt::Display + 'static,
{
    stream
        .scan(SseFrameDecoder::default(), |decoder, item| {
            let out: Vec<Result<CanonicalEvent, String>> = match item {
                Ok(bytes) => decoder.push(&bytes).into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e.to_string())],
            };
            futures_util::future::ready(Some(out))
        })
        .flat_map(stream::iter)
        .boxed()
}
