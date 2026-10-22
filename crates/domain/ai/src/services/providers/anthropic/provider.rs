//! Anthropic provider client construction, request plumbing and its
//! `ProviderClient` implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{generation, search};
use crate::error::Result;
use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk};
use crate::models::tools::ToolCall;
use crate::services::providers::canonical_bridge::tools_to_canonical;
use crate::services::providers::http_client::build_client;
use crate::services::providers::{
    GenerationParams, ModelPricing, ProviderClient, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams, catalog_default_model, catalog_pricing, catalog_supports_model,
};
use crate::services::schema::ProviderCapabilities;
use crate::services::upstream::UpstreamTarget;
use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;
use systemprompt_manifest::services::ProviderModel;
use systemprompt_manifest::services::providers::upstream_model_in;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};

#[derive(Debug)]
pub struct AnthropicProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) web_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl AnthropicProvider {
    pub fn with_target(target: UpstreamTarget) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            target,
            web_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        }
    }

    pub(crate) fn upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        upstream_model_in(&self.models, requested)
    }

    pub const fn with_web_search(mut self) -> Self {
        self.web_search_enabled = true;
        self
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<ProviderModel>) -> Self {
        self.models = models;
        self
    }

    #[must_use]
    pub fn with_default_model(mut self, model: Option<String>) -> Self {
        self.default_model_override = model;
        self
    }
}

#[async_trait]
impl ProviderClient for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::anthropic()
    }

    fn supports_model(&self, model: &str) -> bool {
        catalog_supports_model(&self.models, model)
    }

    fn supports_sampling(&self, _sampling: Option<&SamplingParams>) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        catalog_default_model(&self.models, self.default_model_override.as_deref())
    }

    fn get_pricing(&self, model: &str) -> Option<ModelPricing> {
        catalog_pricing(&self.models, model)
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        generation::generate(self, params).await
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        generation::generate_with_tools(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn generate_stream(
        &self,
        params: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        self.create_stream_request(params, None).await
    }

    async fn generate_with_tools_stream(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let tools = tools_to_canonical(params.tools);
        self.create_stream_request(params.base, Some(tools)).await
    }

    fn supports_google_search(&self) -> bool {
        self.web_search_enabled
    }

    async fn generate_with_google_search(
        &self,
        params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let search_params = search::SearchParams::new(
            params.base.messages,
            params.base.max_output_tokens,
            params.base.model,
        );
        let search_params = if let Some(sampling) = params.base.sampling {
            search_params.with_sampling(sampling)
        } else {
            search_params
        };
        search::generate_with_web_search(self, search_params).await
    }
}
