//! Anthropic SSE on the gateway side.
//!
//! Canonical events for the translated path, raw bytes for passthrough. Frame
//! decoding is the shared [`anthropic::SseFrameDecoder`], which the in-process
//! AI service reads too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::StreamExt;
use systemprompt_models::wire::anthropic;

use super::super::super::canonical_response::CanonicalEvent;

pub fn sse_to_canonical_events<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<CanonicalEvent, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    anthropic::sse_to_canonical_events(stream)
}

pub(in crate::services::gateway) fn raw_sse_stream<S>(
    stream: S,
) -> futures_util::stream::BoxStream<'static, Result<bytes::Bytes, String>>
where
    S: futures_util::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
{
    stream.map(|chunk| chunk.map_err(|e| e.to_string())).boxed()
}
