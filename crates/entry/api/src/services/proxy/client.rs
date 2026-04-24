use systemprompt_models::net::{
    HTTP_KEEPALIVE, HTTP_POOL_IDLE_TIMEOUT, HTTP_STREAM_CONNECT_TIMEOUT,
};

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
                .connect_timeout(HTTP_STREAM_CONNECT_TIMEOUT)
                .tcp_keepalive(Some(HTTP_KEEPALIVE))
                .pool_idle_timeout(HTTP_POOL_IDLE_TIMEOUT)
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
