//! Time and size budgets for the MCP streamable-HTTP client.
//!
//! [`McpClientLimits`] bounds how long a request may wait for its response
//! and its JSON body, and how long a response stream may stay silent. JSON
//! bodies are read through [`read_capped`], which refuses a body past its byte
//! ceiling before it is buffered in full.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::client::challenge::McpTransportError;
use bytes::{Bytes, BytesMut};
use serde::de::DeserializeOwned;
use std::future::Future;
use std::time::Duration;
use tokio::time::Instant;

pub const DEFAULT_MCP_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
pub const DEFAULT_MCP_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
pub const MAX_RESOURCE_METADATA_BYTES: usize = 64 * 1024;

/// Deadlines applied to every request made by `HttpClientWithContext`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpClientLimits {
    request_timeout: Duration,
    stream_idle_timeout: Duration,
}

impl Default for McpClientLimits {
    fn default() -> Self {
        Self {
            request_timeout: DEFAULT_MCP_REQUEST_TIMEOUT,
            stream_idle_timeout: DEFAULT_MCP_STREAM_IDLE_TIMEOUT,
        }
    }
}

impl McpClientLimits {
    #[must_use]
    pub const fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_stream_idle_timeout(mut self, timeout: Duration) -> Self {
        self.stream_idle_timeout = timeout;
        self
    }

    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    #[must_use]
    pub const fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle_timeout
    }

    pub(super) fn deadline(&self) -> Instant {
        Instant::now() + self.request_timeout
    }

    pub(super) async fn within<T>(
        &self,
        deadline: Instant,
        future: impl Future<Output = Result<T, McpTransportError>>,
    ) -> Result<T, McpTransportError> {
        tokio::time::timeout_at(deadline, future)
            .await
            .map_err(|_elapsed| McpTransportError::TimedOut {
                after: self.request_timeout,
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
