//! Time and size budgets for the MCP streamable-HTTP client.
//!
//! [`McpClientLimits`] bounds how long a request may wait for its response
//! and its JSON body, how long connecting may take, and how long a response
//! stream may stay silent. It is built from the `ai.mcp.resilience` profile
//! settings. JSON bodies are read through a capped reader that refuses a body
//! past its byte ceiling before it is buffered in full.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::client::challenge::McpTransportError;
use bytes::{Bytes, BytesMut};
use serde::de::DeserializeOwned;
use std::future::Future;
use std::time::Duration;
use systemprompt_manifest::services::{McpConfig, ResilienceSettings};
use tokio::time::Instant;

pub const MAX_RESOURCE_METADATA_BYTES: usize = 64 * 1024;

/// Deadlines applied to every request made by `HttpClientWithContext`.
///
/// Built from the `ai.mcp.resilience` settings, the same policy the tool
/// provider's resilience guard enforces around each call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpClientLimits {
    request: Duration,
    connect: Duration,
    stream_idle: Duration,
}

impl From<&ResilienceSettings> for McpClientLimits {
    fn from(settings: &ResilienceSettings) -> Self {
        Self {
            request: Duration::from_millis(settings.request_timeout_ms),
            connect: Duration::from_millis(settings.connect_timeout_ms),
            stream_idle: Duration::from_millis(settings.stream_idle_timeout_ms),
        }
    }
}

impl Default for McpClientLimits {
    fn default() -> Self {
        Self::from(&McpConfig::default().resilience)
    }
}

impl McpClientLimits {
    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        self.request
    }

    pub(super) const fn connect_timeout(&self) -> Duration {
        self.connect
    }

    pub(super) const fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle
    }

    pub(super) fn deadline(&self) -> Instant {
        Instant::now() + self.request
    }

    pub(super) async fn within<T>(
        &self,
        deadline: Instant,
        future: impl Future<Output = Result<T, McpTransportError>>,
    ) -> Result<T, McpTransportError> {
        tokio::time::timeout_at(deadline, future)
            .await
            .map_err(|_elapsed| McpTransportError::TimedOut {
                after: self.request,
            })?
    }
}

pub(super) async fn read_capped(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Bytes, McpTransportError> {
    let too_large = McpTransportError::ResponseTooLarge { limit };
    if response
        .content_length()
        .is_some_and(|len| usize::try_from(len).map_or(true, |len| len > limit))
    {
        return Err(too_large);
    }
    let mut body = BytesMut::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(too_large);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

pub(super) async fn read_capped_json<T: DeserializeOwned>(
    response: reqwest::Response,
    limit: usize,
) -> Result<T, McpTransportError> {
    let body = read_capped(response, limit).await?;
    Ok(serde_json::from_slice(&body)?)
}
