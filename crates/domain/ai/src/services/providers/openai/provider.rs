use reqwest::Client;

#[derive(Debug)]
pub struct OpenAiProvider {
    pub(crate) client: Client,
    pub(crate) api_key: String,
    pub(crate) endpoint: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            endpoint: "https://api.openai.com/v1".to_string(),
        }
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            endpoint,
        }
    }
}
