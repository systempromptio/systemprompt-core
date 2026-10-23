//! Gemini provider client construction, request plumbing and its
//! `ProviderClient` implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{generation, search, streaming, tools, transport};
use crate::error::Result;
use crate::models::ai::{
    AiMessage, AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk,
};
use crate::models::tools::ToolCall;
use crate::services::providers::served_model::{ServedModel, upstream_model_in};
use crate::services::providers::{
    GenerationParams, ModelPricing, ProviderClient, SchemaGenerationParams, SearchGenerationParams,
    ToolGenerationParams, ToolResultsParams, catalog_default_model, catalog_pricing,
    catalog_supports_model,
};
use crate::services::schema::ProviderCapabilities;
use crate::services::upstream::UpstreamTarget;
use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use std::pin::Pin;

#[derive(Debug)]
pub struct GeminiProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) google_search_enabled: bool,
    pub(crate) models: Vec<ServedModel>,
    pub(crate) default_model_override: Option<String>,
}

impl GeminiProvider {
    pub fn with_target(target: UpstreamTarget) -> Result<Self> {
        Ok(Self {
            client: transport::build_client()?,
            target,
            google_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        })
    }

    pub(crate) fn upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        upstream_model_in(&self.models, requested)
    }

    pub const fn with_google_search(mut self) -> Self {
        self.google_search_enabled = true;
        self
    }

    #[must_use]
    pub fn with_models(mut self, models: Vec<ServedModel>) -> Self {
        self.models = models;
        self
    }

    #[must_use]
    pub fn with_default_model(mut self, model: Option<String>) -> Self {
        self.default_model_override = model;
        self
    }

    pub const fn has_google_search(&self) -> bool {
        self.google_search_enabled
    }

    pub async fn generate_with_code_execution(
        &self,
        messages: &[AiMessage],
        sampling: Option<&SamplingParams>,
        max_output_tokens: u32,
        model: &str,
    ) -> Result<super::code_execution::CodeExecutionResponse> {
        super::code_execution::generate_with_code_execution(
            self,
            messages,
            sampling,
            max_output_tokens,
            model,
        )
        .await
    }
}

#[async_trait]
impl ProviderClient for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::gemini()
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

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_google_search(&self) -> bool {
        self.google_search_enabled
    }

    async fn generate(&self, params: GenerationParams<'_>) -> Result<AiResponse> {
        generation::generate(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    async fn generate_with_tools(
        &self,
        params: ToolGenerationParams<'_>,
    ) -> Result<(AiResponse, Vec<ToolCall>)> {
        let mut builder = tools::ToolRequestParams::builder(
            params.base.messages,
            &params.tools,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        tools::generate_with_tools(self, builder.build()).await
    }

    async fn generate_with_tool_results(
        &self,
        params: ToolResultsParams<'_>,
    ) -> Result<AiResponse> {
        let mut builder = tools::ToolResultParams::builder(
            params.base.messages,
            params.tool_calls,
            params.tool_results,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        tools::generate_with_tool_results(self, builder.build()).await
    }

    async fn generate_stream(
        &self,
        params: GenerationParams<'_>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        streaming::generate_stream(self, params).await
    }

    async fn generate_with_google_search(
        &self,
        params: SearchGenerationParams<'_>,
    ) -> Result<SearchGroundedResponse> {
        let mut builder = search::SearchParams::builder(
            params.base.messages,
            params.base.max_output_tokens,
            params.base.model,
        );
        if let Some(sampling) = params.base.sampling {
            builder = builder.with_sampling(sampling);
        }
        if let Some(urls) = params.urls {
            builder = builder.with_urls(urls);
        }
        search::generate_with_google_search(self, builder.build()).await
    }
}
