#[derive(Debug, Clone)]
pub struct ClientPool {
    client: reqwest::Client,
}

impl Default for ClientPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientPool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to build HTTP client with timeout, using default");
                    reqwest::Client::new()
                }),
        }
    }

    pub fn get_default_client(&self) -> reqwest::Client {
        self.client.clone()
    }
}
