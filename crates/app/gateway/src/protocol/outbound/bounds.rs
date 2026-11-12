//! Time and size bounds on every upstream provider call.
//!
//! [`UpstreamHttp`] pairs the gateway's outbound client with its
//! [`UpstreamBounds`]: a total deadline over the whole exchange (headers and
//! body), an idle deadline between body chunks, a byte cap on a buffered body,
//! and, for a streaming call only, a first-byte deadline on each attempt's
//! response headers. A non-streaming call sends no header until its generation
//! is complete, so its only bound before the body is the total deadline: the
//! [`ResponseMode`] a call is sent with decides which deadlines apply, and
//! [`ResponseMode::Buffered`] has no first-byte deadline to carry. A
//! stalled or oversized upstream therefore ends as a typed error instead of
//! holding a connection, a quota reservation and a pending audit row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use futures_util::stream::{self, BoxStream};
use thiserror::Error;

/// Whether an upstream call's response is streamed or returned whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseMode {
    Buffered,
    Streaming,
}

impl ResponseMode {
    #[must_use]
    pub const fn of(stream: bool) -> Self {
        if stream {
            Self::Streaming
        } else {
            Self::Buffered
        }
    }
}

/// The deadlines and size caps applied to an upstream call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpstreamBounds {
    pub stream_first_byte: Duration,
    pub idle: Duration,
    pub total: Duration,
    pub max_buffered_bytes: usize,
    pub max_error_body_bytes: usize,
}

impl UpstreamBounds {
    pub const PRODUCTION: Self = Self {
        stream_first_byte: Duration::from_secs(600),
        idle: Duration::from_secs(300),
        total: Duration::from_secs(3600),
        max_buffered_bytes: 32 * 1024 * 1024,
        max_error_body_bytes: 64 * 1024,
    };

    #[must_use]
    pub const fn header_deadline(&self, mode: ResponseMode) -> Option<Duration> {
        match mode {
            ResponseMode::Buffered => None,
            ResponseMode::Streaming => Some(self.stream_first_byte),
        }
    }
}

/// The gateway's outbound HTTP client and the bounds every call through it
/// carries.
#[derive(Debug, Clone)]
pub struct UpstreamHttp {
    client: reqwest::Client,
    bounds: UpstreamBounds,
}

impl UpstreamHttp {
    #[must_use]
    pub const fn new(client: reqwest::Client, bounds: UpstreamBounds) -> Self {
        Self { client, bounds }
    }

    #[must_use]
    pub const fn bounds(&self) -> &UpstreamBounds {
        &self.bounds
    }

    pub fn post(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.post(url).timeout(self.bounds.total)
    }
}

/// Why an upstream body could not be read within its bounds.
#[derive(Debug, Error)]
pub enum UpstreamBodyError {
    #[error("upstream body sent nothing for {}s", .after.as_secs())]
    Idle { after: Duration },
    #[error("upstream body exceeds the {limit}-byte cap")]
    TooLarge { limit: usize },
    #[error("upstream body read failed")]
    Transport(#[source] reqwest::Error),
}

pub(crate) async fn read_capped(
    response: reqwest::Response,
    idle: Duration,
    limit: usize,
) -> Result<Bytes, UpstreamBodyError> {
    if response
        .content_length()
        .is_some_and(|len| len > u64::try_from(limit).unwrap_or(u64::MAX))
    {
        return Err(UpstreamBodyError::TooLarge { limit });
    }
    let mut body = idle_bounded(response, idle);
    let mut buffer = BytesMut::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        if buffer.len().saturating_add(chunk.len()) > limit {
            return Err(UpstreamBodyError::TooLarge { limit });
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer.freeze())
}

pub(crate) fn idle_bounded(
    response: reqwest::Response,
    idle: Duration,
) -> BoxStream<'static, Result<Bytes, UpstreamBodyError>> {
    stream::unfold(
        Some(response.bytes_stream().boxed()),
        move |state| async move {
            let mut inner = state?;
            match tokio::time::timeout(idle, inner.next()).await {
                Err(_) => Some((Err(UpstreamBodyError::Idle { after: idle }), None)),
                Ok(None) => None,
                Ok(Some(Err(e))) => Some((Err(UpstreamBodyError::Transport(e)), None)),
                Ok(Some(Ok(chunk))) => Some((Ok(chunk), Some(inner))),
            }
        },
    )
    .boxed()
}
