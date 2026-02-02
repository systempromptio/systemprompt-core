use reqwest::Client;

#[derive(Debug)]
pub struct AnthropicProvider {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) endpoint: String,
    pub(crate) web_search_enabled: bool,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            endpoint: "https://api.anthropic.com/v1".to_string(),
            web_search_enabled: false,
        }
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self {
            client: Client::new(),
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
