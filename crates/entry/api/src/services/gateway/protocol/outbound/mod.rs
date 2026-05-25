pub mod anthropic;
pub mod openai_chat;
pub mod openai_responses;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::stream::BoxStream;
use systemprompt_models::profile::GatewayRoute;

use super::canonical::CanonicalRequest;
use super::canonical_response::{CanonicalEvent, CanonicalResponse};

#[derive(Debug)]
pub struct OutboundCtx<'a> {
    pub route: &'a GatewayRoute,
    pub endpoint: &'a str,
    pub api_key: &'a str,
    pub request: &'a CanonicalRequest,
    pub upstream_model: &'a str,
}

#[expect(missing_debug_implementations, reason = "holds borrowed pool/transaction/service handles that do not implement Debug")]
pub enum OutboundOutcome {
    Buffered(CanonicalResponse),
    Streaming(BoxStream<'static, Result<CanonicalEvent, String>>),
}

// Why: #[async_trait] is required — the upstream registry stores adapters as
// `Arc<dyn OutboundAdapter>`, so the trait must stay dyn-compatible.
#[async_trait]
pub trait OutboundAdapter: Send + Sync {
    fn provider_tag(&self) -> &'static str;
    async fn send(&self, ctx: OutboundCtx<'_>) -> Result<OutboundOutcome>;
}

#[derive(Debug, Clone, Copy)]
pub struct OutboundAdapterRegistration {
    pub tag: &'static str,
    pub factory: fn() -> Arc<dyn OutboundAdapter>,
}

inventory::collect!(OutboundAdapterRegistration);
