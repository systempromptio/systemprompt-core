use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub fn build_http_client() -> Result<Client> {
    Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .build()
        .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))
}
