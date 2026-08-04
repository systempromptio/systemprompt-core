//! Outbound protocol adapters: canonical model to upstream provider.
//!
//! The [`OutboundAdapter`] trait sends a [`CanonicalRequest`] to an upstream
//! provider and yields an [`OutboundOutcome`] — a buffered response or a stream
//! of canonical events. Adapters register themselves via
//! [`OutboundAdapterRegistration`] (collected by `inventory`) so the upstream
//! registry can resolve one by provider tag. Implementations cover Anthropic,
//! `OpenAI` Chat Completions, and `OpenAI` Responses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anthropic;
pub mod gemini;
pub mod openai_chat;
pub mod openai_responses;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use systemprompt_models::profile::GatewayRoute;
use systemprompt_models::services::ai::ModelLimits;
use thiserror::Error;

use super::canonical::CanonicalRequest;
use super::canonical_response::{CanonicalEvent, CanonicalResponse};

/// Upstream provider failure, carried inside the `anyhow::Error` an adapter
/// returns so the route layer can recover the real HTTP status by downcast
/// instead of flattening every failure to 502.
#[derive(Debug, Error)]
pub enum UpstreamError {
    #[error("{provider} returned {status}: {message}")]
    Status {
        provider: String,
        status: u16,
        message: String,
        /// The provider's error response verbatim.
        ///
        /// Claude Code recovers from several upstream rejections by matching on
        /// the error's own wording and then disabling the rejected capability
        /// for the rest of the conversation. Re-wrapping the error in the
        /// gateway's envelope defeats that even when the status code survives,
        /// so the original bytes are carried here and relayed unchanged.
        body: bytes::Bytes,
        /// `retry-after` as sent by the provider, when present.
        retry_after: Option<String>,
        /// The provider's own request id, for correlating with their support.
        request_id: Option<String>,
    },
    #[error("{provider} request failed: {source}")]
    Transport {
        provider: String,
        #[source]
        source: reqwest::Error,
    },
}

impl UpstreamError {
    /// Builds a [`UpstreamError::Status`] from a non-success upstream response,
    /// preserving the body and the headers a client needs to retry correctly.
    pub async fn from_response(provider: &str, response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(ToOwned::to_owned)
        };
        let retry_after = header("retry-after");
        let request_id = header("request-id").or_else(|| header("x-request-id"));
        let body = response.bytes().await.unwrap_or_default();
        Self::Status {
            provider: provider.to_owned(),
            status,
            message: extract_upstream_message(&String::from_utf8_lossy(&body)),
            body,
            retry_after,
            request_id,
        }
    }
}

// Why: one process-wide client — a client per request would open a fresh
// connection pool and TLS handshake on every gateway call.
pub(in crate::services::gateway) fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

pub(in crate::services::gateway) async fn send_checked(
    provider: &str,
    req: reqwest::RequestBuilder,
) -> Result<reqwest::Response> {
    let response = req.send().await.map_err(|e| {
        anyhow::Error::new(UpstreamError::Transport {
            provider: provider.to_owned(),
            source: e,
        })
    })?;
    if !response.status().is_success() {
        return Err(anyhow::Error::new(
            UpstreamError::from_response(provider, response).await,
        ));
    }
    Ok(response)
}

pub fn extract_upstream_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| body.chars().take(500).collect())
}

#[derive(Debug)]
pub struct OutboundCtx<'a> {
    pub route: &'a GatewayRoute,
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a CanonicalRequest,
    pub upstream_model: &'a str,
    pub model_limits: Option<ModelLimits>,
    /// Inbound headers cleared for verbatim relay, already stripped of every
    /// header that identifies the client, user, or session.
    pub forward_headers: &'a [(String, String)],
    /// The caller's request body, set only when the caller's wire protocol
    /// matches the upstream's and the bytes can be relayed untouched.
    pub raw_body: Option<&'a bytes::Bytes>,
}

#[expect(
    missing_debug_implementations,
    reason = "variants hold streaming bodies that intentionally do not implement Debug"
)]
pub enum OutboundOutcome {
    Buffered(Box<CanonicalResponse>),
    Streaming(BoxStream<'static, Result<CanonicalEvent, String>>),
    /// A non-streaming response relayed byte-for-byte, with a canonical parse
    /// alongside it purely so audit, cost, and safety keep working.
    RawBuffered {
        body: bytes::Bytes,
        content_type: Option<String>,
        canonical: Box<CanonicalResponse>,
    },
    /// A streaming response relayed byte-for-byte. Usage accounting reads a
    /// copy of the frames as they pass; the bytes the client receives are the
    /// provider's own.
    RawStreaming {
        content_type: Option<String>,
        stream: BoxStream<'static, Result<bytes::Bytes, String>>,
    },
}

/// The exact bytes an adapter will put on the wire.
///
/// `raw_lane` records that the bytes started as the caller's own; they are
/// still normalised in place, so they are not byte-identical to what arrived.
#[derive(Debug, Clone)]
pub struct PreparedBody {
    pub bytes: bytes::Bytes,
    pub raw_lane: bool,
}

// Why: #[async_trait] is required — the upstream registry stores adapters as
// `Arc<dyn OutboundAdapter>`, so the trait must stay dyn-compatible.
#[async_trait]
pub trait OutboundAdapter: Send + Sync {
    /// Kept separate from [`OutboundAdapter::send`], and sync and pure, so the
    /// gateway can have governance inspect the same bytes the socket will
    /// carry before it commits to sending them.
    fn build_body(&self, ctx: &OutboundCtx<'_>) -> Result<PreparedBody>;

    async fn send(&self, ctx: OutboundCtx<'_>, body: &PreparedBody) -> Result<OutboundOutcome>;
}

#[derive(Debug, Clone, Copy)]
pub struct OutboundAdapterRegistration {
    pub tag: &'static str,
    pub factory: fn() -> Arc<dyn OutboundAdapter>,
}

inventory::collect!(OutboundAdapterRegistration);
