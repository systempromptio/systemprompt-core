//! Gemini Google Search grounding: builds a canonical request carrying a
//! [`SearchConfig`] (URLs trigger the url-context tool too), renders it with
//! the shared codec, and maps the grounded reply back.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::canonical::SearchConfig;
use systemprompt_models::wire::gemini;

use crate::error::Result;
use crate::models::ai::{AiMessage, SamplingParams, SearchGroundedResponse};
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::GeminiProvider;
use super::transport;

pub(super) struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub urls: Option<Vec<String>>,
}

pub(super) struct SearchParamsBuilder<'a> {
    messages: &'a [AiMessage],
    sampling: Option<&'a SamplingParams>,
    max_output_tokens: u32,
    model: &'a str,
    urls: Option<Vec<String>>,
}

impl<'a> SearchParamsBuilder<'a> {
    pub(super) const fn new(
        messages: &'a [AiMessage],
        max_output_tokens: u32,
        model: &'a str,
    ) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
            urls: None,
        }
    }

    pub(super) const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub(super) fn with_urls(mut self, urls: Vec<String>) -> Self {
        self.urls = Some(urls);
        self
    }

    pub(super) fn build(self) -> SearchParams<'a> {
        SearchParams {
            messages: self.messages,
            sampling: self.sampling,
            max_output_tokens: self.max_output_tokens,
            model: self.model,
            urls: self.urls,
        }
    }
}

impl<'a> SearchParams<'a> {
    pub(super) const fn builder(
        messages: &'a [AiMessage],
        max_output_tokens: u32,
        model: &'a str,
    ) -> SearchParamsBuilder<'a> {
        SearchParamsBuilder::new(messages, max_output_tokens, model)
    }
}

pub(super) async fn generate_with_google_search(
    provider: &GeminiProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();
    let search = SearchConfig {
        max_uses: None,
        context_size: None,
        urls: params.urls.unwrap_or_default(),
    };
    let canonical = CanonicalBuild::new(
        BridgeProvider::Gemini,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_search(Some(search))
    .into_request();

    let body = gemini::build_request_body(&canonical, None);
    let value: Value = transport::post(provider, &body, params.model, false)
        .await?
        .json()
        .await?;
    let parsed = gemini::parse_response(&value, params.model);
    Ok(canonical_bridge::to_search_grounded(start, &parsed))
}
