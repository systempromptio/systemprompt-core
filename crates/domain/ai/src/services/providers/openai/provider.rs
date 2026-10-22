//! `OpenAI` provider client construction, request plumbing and its
//! `ProviderClient` implementation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{generation, search};
use crate::error::Result;
use crate::models::ai::{AiResponse, SamplingParams, SearchGroundedResponse, StreamChunk};
use crate::models::tools::ToolCall;
use crate::services::providers::canonical_bridge::tools_to_canonical;
use crate::services::providers::{
    GenerationParams, ModelPricing, ProviderClient, SchemaGenerationParams, SearchGenerationParams,
    StructuredGenerationParams, ToolGenerationParams, catalog_default_model, catalog_pricing,
    catalog_supports_model,
};
use crate::services::schema::ProviderCapabilities;
use async_trait::async_trait;
use futures::Stream;
use reqwest::{Client, Response};
use std::pin::Pin;

// JSON: protocol boundary — the rendered wire body is dynamic JSON.
use serde_json::Value;
use systemprompt_manifest::services::ProviderModel;
use systemprompt_manifest::services::providers::upstream_model_in;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use systemprompt_wire::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_wire::{WireProtocol, openai_chat, openai_responses};

use crate::services::providers::http_client::build_client;
use crate::services::upstream::UpstreamTarget;

#[derive(Debug)]
pub struct OpenAiProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) web_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl OpenAiProvider {
    pub fn with_target(target: UpstreamTarget) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            target,
            web_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        }
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

    pub(crate) fn upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        upstream_model_in(&self.models, requested)
    }

    // JSON: OpenAI Chat Completions / Responses request body — upstream wire
    // format.
    pub(crate) fn render(&self, canonical: &CanonicalRequest, upstream_model: &str) -> Value {
        match self.target.wire() {
            WireProtocol::OpenAiResponses => {
                openai_responses::build_request_body(canonical, upstream_model, None)
            },
            _ => openai_chat::build_request_body(canonical, upstream_model, None),
        }
    }

    // JSON: OpenAI Chat Completions / Responses response body — upstream wire
    // format.
    pub(crate) fn parse(&self, value: &Value, model: &str) -> Result<CanonicalResponse> {
        Ok(match self.target.wire() {
            WireProtocol::OpenAiResponses => openai_responses::parse_response_object(value, model)?,
            _ => openai_chat::parse_response(value, model)?,
        })
    }

    // JSON: OpenAI Chat Completions / Responses request body — upstream wire
    // format.
    pub(crate) async fn post(
        &self,
        wire: WireProtocol,
        mut body: Value,
        upstream_model: &str,
        stream: bool,
    ) -> Result<Response> {
        let call = self.target.call().await?;
        call.finish_value(wire, &mut body);
        let mut request = self.client.post(call.url(wire, upstream_model, stream));
        for (name, value) in call.headers(wire) {
            request = request.header(name, value);
        }
        let response = request.json(&body).send().await?;
        if !response.status().is_success() {
            return Err(crate::error::AiError::from_error_response("openai", response).await);
        }
        Ok(response)
    }
}

#[async_trait]
impl ProviderClient for OpenAiProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::openai()
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

    async fn generate_structured(
        &self,
        params: StructuredGenerationParams<'_>,
    ) -> Result<AiResponse> {
        generation::generate_structured(self, params).await
    }

    async fn generate_with_schema(&self, params: SchemaGenerationParams<'_>) -> Result<AiResponse> {
        generation::generate_with_schema(self, params).await
    }

    fn supports_json_mode(&self) -> bool {
        true
    }

    fn supports_structured_output(&self) -> bool {
        true
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
