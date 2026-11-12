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
use crate::sse::{FrameLedger, SseFramer};

const WIRE: &str = "anthropic";

/// How one Anthropic `data:` payload read against the wire envelope (a JSON
/// object whose `type` is a string; an unknown `type` still decodes).
#[derive(Debug)]
pub enum DecodedFrame {
    Benign,
    Events(Vec<CanonicalEvent>),
    Dropped,
}

#[derive(Debug, Default)]
pub struct SseFrameDecoder {
    framer: SseFramer,
    codec: AnthropicStreamState,
    ledger: FrameLedger,
}

impl SseFrameDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<CanonicalEvent, WireStreamError>> {
        let mut events = Vec::new();
        for frame in self.framer.push(chunk) {
            match frame {
                Ok(frame) => match self.decode_data(&frame.data) {
                    DecodedFrame::Benign => {},
                    DecodedFrame::Events(decoded) => events.extend(decoded.into_iter().map(Ok)),
                    DecodedFrame::Dropped => events.push(Ok(CanonicalEvent::FrameDropped {
                        bytes: frame.data.len(),
                    })),
                },
                Err(e) => events.push(Err(e.into())),
            }
        }
        events
    }

    pub fn decode_data(&mut self, data: &str) -> DecodedFrame {
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            return DecodedFrame::Benign;
        }
        match serde_json::from_str::<Value>(data) {
            Ok(value) if value.get("type").is_some_and(Value::is_string) => {
                self.ledger.decoded();
                DecodedFrame::Events(self.codec.events_from_sse(&value))
            },
            Ok(_) => {
                self.ledger
                    .dropped(WIRE, "frame has no string `type`", data.len());
                DecodedFrame::Dropped
            },
            Err(e) => {
                self.ledger.dropped_json(WIRE, &e, data.len());
                DecodedFrame::Dropped
            },
        }
    }

    #[must_use]
    pub const fn finish(&self) -> Option<WireStreamError> {
        self.ledger.finish()
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
        .map(|chunk| Some(chunk.map_err(WireStreamError::transport)))
        .chain(stream::once(futures_util::future::ready(None)))
        .scan(SseFrameDecoder::default(), |decoder, item| {
            let out: Vec<Result<CanonicalEvent, WireStreamError>> = match item {
                Some(Ok(bytes)) => decoder.push(&bytes),
                Some(Err(e)) => vec![Err(e)],
                None => decoder.finish().map(Err).into_iter().collect(),
            };
            futures_util::future::ready(Some(out))
        })
        .flat_map(stream::iter)
        .boxed()
}
