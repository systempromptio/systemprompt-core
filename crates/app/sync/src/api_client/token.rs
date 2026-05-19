//! `Service`-JWT acquisition for the direct-sync path.
//!
//! On the direct-sync path the deployment's `/api/v1/sync/*` routes are
//! governed by the authz framework and require a `Service`-type JWT. This
//! module exchanges the `sync_client_secret` for that JWT via the
//! `client_credentials` grant and caches it for the run.

use serde::Deserialize;
use systemprompt_identifiers::ClientId;

use super::{SyncApiClient, response};
use crate::error::{SyncError, SyncResult};

#[derive(Debug, Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
}

pub(super) const fn is_unauthorized(error: &SyncError) -> bool {
    matches!(error, SyncError::Unauthorized)
        || matches!(error, SyncError::ApiError { status: 401, .. })
}

impl SyncApiClient {
    /// On the direct-sync path, exchange `sync_client_secret` for a `Service`
    /// JWT via the `client_credentials` grant and cache it for the run;
    /// `force_refresh` discards the cache after a `401`. On the cloud-relay
    /// path returns the cloud `api_token` unchanged.
    pub(super) async fn bearer_token(&self, force_refresh: bool) -> SyncResult<String> {
        let (Some(hostname), Some(secret)) = (&self.hostname, &self.sync_client_secret) else {
            return Ok(self.token.clone());
        };

        let mut cached = self.cached_sync_token.lock().await;
        if !force_refresh {
            if let Some(token) = cached.as_ref() {
                return Ok(token.clone());
            }
        }

        let token = self.exchange_client_credentials(hostname, secret).await?;
        *cached = Some(token.clone());
        drop(cached);
        Ok(token)
    }

    async fn exchange_client_credentials(
        &self,
        hostname: &str,
        secret: &str,
    ) -> SyncResult<String> {
        let url = format!("https://{hostname}/api/v1/core/oauth/token");
        let client_id = ClientId::sync();
        let response = self
            .client
            .post(&url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", client_id.as_str()),
                ("client_secret", secret),
                ("scope", "service"),
            ])
            .send()
            .await?;

        let parsed: TokenExchangeResponse = response::handle_json(response).await?;
        Ok(parsed.access_token)
    }
}
