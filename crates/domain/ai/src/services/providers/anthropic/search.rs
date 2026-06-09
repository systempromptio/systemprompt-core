//! Anthropic server-side web search: builds a canonical request carrying a
//! [`SearchConfig`], renders it with the shared codec (which adds the
//! `web_search` server tool), and maps the grounded reply back.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::anthropic;
use systemprompt_models::wire::canonical::SearchConfig;

use crate::error::Result;
use crate::models::ai::{AiMessage, SamplingParams, SearchGroundedResponse};
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::AnthropicProvider;
use super::request::post_body;

const DEFAULT_MAX_USES: u32 = 5;

#[derive(Debug)]
pub struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub max_uses: Option<u32>,
}

impl<'a> SearchParams<'a> {
    pub const fn new(messages: &'a [AiMessage], max_output_tokens: u32, model: &'a str) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
            max_uses: None,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub const fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = Some(max_uses);
        self
    }
}

pub async fn generate_with_web_search(
    provider: &AnthropicProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();
    let search = SearchConfig {
        max_uses: Some(params.max_uses.unwrap_or(DEFAULT_MAX_USES)),
        context_size: None,
        urls: Vec::new(),
    };
    let canonical = CanonicalBuild::new(
        BridgeProvider::Anthropic,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_search(Some(search))
    .into_request();

    let body = anthropic::build_request_body(&canonical, params.model, None);
    let value: Value = post_body(provider, &body).await?.json().await?;
    let parsed = anthropic::parse_response(&value, params.model);
    Ok(canonical_bridge::to_search_grounded(start, &parsed))
}
