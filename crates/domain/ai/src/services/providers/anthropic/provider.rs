//! Anthropic provider client construction and request plumbing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Client;
use systemprompt_identifiers::ProviderId;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use systemprompt_models::services::providers::upstream_model_in;
use systemprompt_models::services::{ProviderModel, WireProtocol};

use crate::services::providers::http_client::build_client;
use crate::services::upstream::UpstreamTarget;

const DEFAULT_ENDPOINT: &str = "https://api.anthropic.com/v1";

#[derive(Debug)]
pub struct AnthropicProvider {
    pub(crate) client: Client,
    pub(crate) target: UpstreamTarget,
    pub(crate) web_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self::with_endpoint(api_key, DEFAULT_ENDPOINT.to_owned())
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self::with_target(UpstreamTarget::api_key(
            ProviderId::new("anthropic"),
            WireProtocol::Anthropic,
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
