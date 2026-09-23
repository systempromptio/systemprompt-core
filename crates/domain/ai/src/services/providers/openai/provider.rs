//! `OpenAI` provider client construction and request plumbing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::{Client, Response};
// JSON: protocol boundary — the rendered wire body is dynamic JSON.
use serde_json::Value;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use systemprompt_models::services::providers::upstream_model_in;
use systemprompt_models::services::{ProviderModel, WireProtocol};
use systemprompt_models::wire::canonical::{CanonicalRequest, CanonicalResponse};
use systemprompt_models::wire::{openai_chat, openai_responses};

use crate::error::Result;
use crate::services::providers::http_client::build_client;
use crate::services::upstream::UpstreamTarget;

const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1";

#[derive(Debug)]
pub struct OpenAiProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) web_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self::with_endpoint(api_key, DEFAULT_ENDPOINT.to_owned())
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self::with_target(UpstreamTarget::api_key(
            "openai",
            WireProtocol::OpenAiChat,
            endpoint,
            api_key,
        ))
    }

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

    pub(crate) fn render(&self, canonical: &CanonicalRequest, upstream_model: &str) -> Value {
        match self.target.wire() {
            WireProtocol::OpenAiResponses => {
                openai_responses::build_request_body(canonical, upstream_model, None)
            },
            _ => openai_chat::build_request_body(canonical, upstream_model, None),
        }
    }

    pub(crate) fn parse(&self, value: &Value, model: &str) -> Result<CanonicalResponse> {
        Ok(match self.target.wire() {
            WireProtocol::OpenAiResponses => openai_responses::parse_response_object(value, model)?,
            _ => openai_chat::parse_response(value, model)?,
        })
    }

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
