//! `OpenAI` server-side web search via the Responses API: builds a canonical
//! request carrying a [`SearchConfig`], renders it with the shared
//! `openai_responses` codec, and maps the grounded reply back.

use std::time::Instant;

use serde_json::Value;
use systemprompt_models::wire::canonical::SearchConfig;
use systemprompt_models::wire::openai_responses;

use crate::error::Result;
use crate::models::ai::{AiMessage, SamplingParams, SearchGroundedResponse};
use crate::services::providers::canonical_bridge::{self, BridgeProvider, CanonicalBuild};

use super::provider::OpenAiProvider;

const DEFAULT_CONTEXT_SIZE: &str = "medium";

#[derive(Debug)]
pub struct SearchParams<'a> {
    pub messages: &'a [AiMessage],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

impl<'a> SearchParams<'a> {
    pub const fn new(messages: &'a [AiMessage], max_output_tokens: u32, model: &'a str) -> Self {
        Self {
            messages,
            sampling: None,
            max_output_tokens,
            model,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }
}

pub async fn generate_with_web_search(
    provider: &OpenAiProvider,
    params: SearchParams<'_>,
) -> Result<SearchGroundedResponse> {
    let start = Instant::now();
    let search = SearchConfig {
        max_uses: None,
        context_size: Some(DEFAULT_CONTEXT_SIZE.to_owned()),
        urls: Vec::new(),
    };
    let canonical = CanonicalBuild::new(
        BridgeProvider::OpenAi,
        params.messages,
        params.model,
        params.max_output_tokens,
    )
    .with_sampling(params.sampling)
    .with_search(Some(search))
    .into_request();

    let body = openai_responses::build_request_body(&canonical, params.model);
    let response = provider
        .client
        .post(format!("{}/responses", provider.endpoint))
        .bearer_auth(&provider.api_key)
        .json(&body)
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(crate::error::AiError::from_error_response("openai", response).await);
    }
    let value: Value = response.json().await?;
    let parsed = openai_responses::parse_response_object(&value, params.model);
    Ok(canonical_bridge::to_search_grounded(start, &parsed))
}
