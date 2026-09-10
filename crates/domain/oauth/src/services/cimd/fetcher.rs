//! CIMD metadata document fetcher (HTTPS GET).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use crate::models::cimd::CimdMetadata;
use reqwest::Client;
use systemprompt_identifiers::ClientId;
use systemprompt_models::net::{GuardedClientConfig, HTTP_AUTH_VERIFY_TIMEOUT, guarded_client};

#[derive(Debug)]
pub struct CimdFetcher {
    client: Client,
}

impl CimdFetcher {
    pub fn new() -> Result<Self> {
        let client = guarded_client(
            &GuardedClientConfig::default()
                .with_timeout(HTTP_AUTH_VERIFY_TIMEOUT)
                .with_user_agent(concat!("systemprompt.io-OS/", env!("CARGO_PKG_VERSION"))),
        )
        .map_err(|e| {
            crate::error::OauthError::CimdFetch(format!("Failed to build HTTP client: {e}"))
        })?;

        Ok(Self { client })
    }

    pub async fn fetch_metadata(&self, client_id: &ClientId) -> Result<CimdMetadata> {
        let client_id_str = client_id.as_str();
        if !client_id_str.starts_with("https://") {
            return Err(crate::error::OauthError::InvalidClientMetadata(
                "CIMD client_id must be HTTPS URL".to_owned(),
            ));
        }

        let response = self
            .client
            .get(client_id_str)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| {
                crate::error::OauthError::CimdFetch(format!(
                    "Failed to fetch CIMD metadata from {client_id_str}: {e}"
                ))
            })?;

        if !response.status().is_success() {
            return Err(crate::error::OauthError::CimdFetch(format!(
                "Failed to fetch CIMD metadata: HTTP {} from {}",
                response.status(),
                client_id_str
            )));
        }

        let metadata: CimdMetadata = response.json().await.map_err(|e| {
            crate::error::OauthError::CimdFetch(format!(
                "Invalid CIMD metadata JSON from {client_id_str}: {e}"
            ))
        })?;

        if metadata.client_id.as_str() != client_id_str {
            return Err(crate::error::OauthError::InvalidClientMetadata(format!(
                "CIMD metadata client_id mismatch: expected '{}', got '{}'",
                client_id_str, metadata.client_id
            )));
        }

        metadata.validate()?;

        Ok(metadata)
    }
}
