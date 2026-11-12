//! The upstream's own answer, returned to the client as it arrived.
//!
//! A hop whose policy returns the response whole is not re-rendered: the
//! status, the headers and the body bytes the upstream sent are what the
//! client receives. The gateway still has to account for the exchange, so it
//! reads a copy with the hop's own wire codec ([`WireCodec`]): a buffered body
//! is parsed once, and a streamed body is teed, the client's bytes on one side
//! and the same bytes through the wire's event decoder on the other. Every
//! wire is relayed by this one path; an adapter names its codec and nothing
//! else.
//!
//! The request is sent once: any answer, success or not, is
//! the upstream's and is handed back with its headers
//! ([`UpstreamError::Relayed`]). A body that cannot be read whole is refused
//! rather than cut short.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use http::HeaderMap;
// JSON: protocol boundary — an upstream response body is dynamic wire JSON.
use serde_json::Value;
use systemprompt_wire::error::{WireParseError, WireStreamError};
use systemprompt_wire::upstream::is_returned_response_header;

use super::bounds::{idle_bounded, read_capped};
use super::{OutboundCtx, OutboundError, OutboundOutcome, ResponseMode, UpstreamError};
use crate::protocol::canonical::{CanonicalEvent, CanonicalResponse};

pub type ByteStream = BoxStream<'static, Result<Bytes, WireStreamError>>;
pub type EventStream = BoxStream<'static, Result<CanonicalEvent, WireStreamError>>;

/// How one wire's response is read for accounting.
#[derive(Debug, Clone, Copy)]
pub struct WireCodec {
    pub wire: &'static str,
    // JSON: an upstream response body in the wire's own dynamic format.
    pub parse: fn(&Value, &str) -> Result<CanonicalResponse, WireParseError>,
    pub events: fn(ByteStream, String) -> EventStream,
}

/// An upstream success returned whole: its headers and its body.
#[expect(
    missing_debug_implementations,
    reason = "the body holds streams that do not implement Debug"
)]
pub struct Relayed {
    pub headers: HeaderMap,
    pub body: RelayedBody,
}

/// The client's bytes beside the copy the gateway accounts from.
#[expect(
    missing_debug_implementations,
    reason = "the streaming variant holds streams that do not implement Debug"
)]
pub enum RelayedBody {
    Buffered {
        bytes: Bytes,
        canonical: Box<CanonicalResponse>,
    },
    Streaming {
        bytes: ByteStream,
        events: EventStream,
    },
}

#[must_use]
pub fn returned_headers(upstream: &HeaderMap) -> HeaderMap {
    upstream
        .iter()
        .filter(|(name, _)| is_returned_response_header(name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

pub(crate) async fn send_once(
    provider: &str,
    ctx: &OutboundCtx<'_>,
    req: reqwest::RequestBuilder,
) -> Result<reqwest::Response, UpstreamError> {
    let bounds = ctx.http.bounds();
    let sent = match bounds.header_deadline(ResponseMode::of(ctx.request.stream)) {
        None => req.send().await,
        Some(after) => tokio::time::timeout(after, req.send()).await.map_err(
            |_elapsed: tokio::time::error::Elapsed| UpstreamError::Timeout {
                provider: provider.to_owned(),
                after,
            },
        )?,
    };
    let response = sent.map_err(|source| UpstreamError::Transport {
        provider: provider.to_owned(),
        source,
    })?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let headers = returned_headers(response.headers());
    match read_capped(response, bounds.idle, bounds.max_buffered_bytes).await {
        Ok(body) => Err(UpstreamError::Relayed {
            provider: provider.to_owned(),
            status,
            headers: Box::new(headers),
            body: Box::new(body),
        }),
        Err(source) => Err(UpstreamError::UnreadableBody {
            provider: provider.to_owned(),
            status,
            source,
        }),
    }
}

pub(crate) async fn relay(
    ctx: &OutboundCtx<'_>,
    response: reqwest::Response,
    codec: WireCodec,
) -> Result<OutboundOutcome, OutboundError> {
    let headers = returned_headers(response.headers());
    let model = ctx.request.model.as_str();
    if ctx.request.stream {
        let upstream = idle_bounded(response, ctx.http.bounds().idle)
            .map(|chunk| chunk.map_err(WireStreamError::transport))
            .boxed();
        let (bytes, events) = tee(upstream, |copy| (codec.events)(copy, model.to_owned()));
        return Ok(OutboundOutcome::Relayed(Relayed {
            headers,
            body: RelayedBody::Streaming { bytes, events },
        }));
    }
    let bytes = super::read_body(codec.wire, ctx.http, response).await?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|source| OutboundError::DecodeBody {
            wire: codec.wire,
            source,
        })?;
    let canonical = (codec.parse)(&value, model).map_err(|e| {
        super::reject_unparsable_body(ctx.target.provider.as_str(), codec.wire, &e, &bytes)
    })?;
    Ok(OutboundOutcome::Relayed(Relayed {
        headers,
        body: RelayedBody::Buffered {
            bytes,
            canonical: Box::new(canonical),
        },
    }))
}

// Why: the copy ends when the client's side is dropped, which is how the
// decoder learns the upstream is finished; the reader drops the byte stream
// as soon as it ends.
pub fn tee(
    upstream: ByteStream,
    decode: impl FnOnce(ByteStream) -> EventStream,
) -> (ByteStream, EventStream) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
    let copy = stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|chunk| (Ok(chunk), rx))
    })
    .boxed();
    let bytes = upstream
        .inspect(move |chunk| {
            if let Ok(chunk) = chunk
                && tx.send(chunk.clone()).is_err()
            {
                tracing::trace!("relayed stream copy has no reader; accounting ended early");
            }
        })
        .boxed();
    (bytes, decode(copy))
}
