use reqwest::Client;
use systemprompt_models::net::{AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT};

use crate::services::providers::http_client::build_client;

#[derive(Debug)]
pub struct OpenAiProvider {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) endpoint: String,
    pub(crate) web_search_enabled: bool,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            api_key,
            endpoint: "https://api.openai.com/v1".to_owned(),
            web_search_enabled: false,
        }
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self {
            client: build_client(AI_PROVIDER_REQUEST_TIMEOUT, HTTP_CONNECT_TIMEOUT),
            api_key,
            endpoint,
            web_search_enabled: false,
        }
    }

    pub const fn with_web_search(mut self) -> Self {
        self.web_search_enabled = true;
        self
    }
}
