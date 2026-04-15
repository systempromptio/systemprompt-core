use crate::models::cimd::CimdMetadata;
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::time::Duration;
use systemprompt_identifiers::ClientId;

#[derive(Debug)]
pub struct CimdFetcher {
    client: Client,
}

impl CimdFetcher {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("systemprompt.io-OS/2.0")
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        Ok(Self { client })
    }

    pub async fn fetch_metadata(&self, client_id: &ClientId) -> Result<CimdMetadata> {
        let client_id_str = client_id.as_str();
        if !client_id_str.starts_with("https://") {
            return Err(anyhow!("CIMD client_id must be HTTPS URL"));
        }

        let response = self
            .client
            .get(client_id_str)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch CIMD metadata from {client_id_str}: {e}"))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch CIMD metadata: HTTP {} from {}",
                response.status(),
                client_id_str
            ));
        }

        let metadata: CimdMetadata = response
            .json()
            .await
            .map_err(|e| anyhow!("Invalid CIMD metadata JSON from {client_id_str}: {e}"))?;

        if metadata.client_id.as_str() != client_id_str {
            return Err(anyhow!(
                "CIMD metadata client_id mismatch: expected '{}', got '{}'",
                client_id_str,
                metadata.client_id
            ));
        }

        metadata.validate()?;

        Ok(metadata)
    }
}
