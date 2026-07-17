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
    },
    #[error("{provider} request failed: {source}")]
    Transport {
        provider: String,
        #[source]
        source: reqwest::Error,
    },
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
}

#[expect(
    missing_debug_implementations,
    reason = "variants hold streaming bodies that intentionally do not implement Debug"
)]
pub enum OutboundOutcome {
    Buffered(Box<CanonicalResponse>),
    Streaming(BoxStream<'static, Result<CanonicalEvent, String>>),
}

// Why: #[async_trait] is required — the upstream registry stores adapters as
// `Arc<dyn OutboundAdapter>`, so the trait must stay dyn-compatible.
#[async_trait]
pub trait OutboundAdapter: Send + Sync {
    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome>;
}

#[derive(Debug, Clone, Copy)]
pub struct OutboundAdapterRegistration {
    pub tag: &'static str,
    pub factory: fn() -> Arc<dyn OutboundAdapter>,
}

inventory::collect!(OutboundAdapterRegistration);
