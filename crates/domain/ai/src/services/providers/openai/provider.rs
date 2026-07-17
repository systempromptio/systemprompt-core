//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Client;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};
use systemprompt_models::profile::ProviderModel;

use crate::services::providers::http_client::build_client;

#[derive(Debug)]
pub struct OpenAiProvider {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) endpoint: String,
    pub(crate) web_search_enabled: bool,
    pub(crate) models: Vec<ProviderModel>,
    pub(crate) default_model_override: Option<String>,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            api_key,
            endpoint: "https://api.openai.com/v1".to_owned(),
            web_search_enabled: false,
            models: Vec::new(),
            default_model_override: None,
        }
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            api_key,
            endpoint,
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
}
