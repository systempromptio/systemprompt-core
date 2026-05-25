//! Service-JWT acquisition for the direct-sync path via RFC 8693 token
//! exchange.
//!
//! On the direct-sync path the deployment's `/api/v1/sync/*` routes are
//! governed by the authz framework and require a `Service`-type JWT. This
//! module exchanges the operator's existing `api_token` (as the
//! `subject_token`) for that JWT via the RFC 8693
//! `urn:ietf:params:oauth:grant-type:token-exchange` grant against the
//! deployment's `/api/v1/core/oauth/token` endpoint, and caches the result
//! for the run.

use serde::Deserialize;

use super::{SyncApiClient, response};
use crate::error::{SyncError, SyncResult};

const SUBJECT_TOKEN_TYPE_JWT: &str = "urn:ietf:params:oauth:token-type:jwt";
const TOKEN_EXCHANGE_GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";

#[derive(Debug, Deserialize)]
struct TokenExchangeResponse {
    access_token: String,
}

pub(super) const fn is_unauthorized(error: &SyncError) -> bool {
    matches!(error, SyncError::Unauthorized)
        || matches!(error, SyncError::ApiError { status: 401, .. })
}

impl SyncApiClient {
    pub(super) async fn bearer_token(&self, force_refresh: bool) -> SyncResult<String> {
        let Some(origin) = self.direct_sync_origin() else {
            return Ok(self.token.clone());
        };

        let mut cached = self.cached_sync_token.lock().await;
        if !force_refresh {
            if let Some(token) = cached.as_ref() {
                return Ok(token.clone());
            }
        }

        let token = exchange_subject_token_at(&self.client, origin, &self.token).await?;
        *cached = Some(token.clone());
        drop(cached);
        Ok(token)
    }
}

pub async fn exchange_subject_token(
    client: &reqwest::Client,
    hostname: &str,
    operator_token: &str,
) -> SyncResult<String> {
    let origin = format!("https://{hostname}");
    exchange_subject_token_at(client, &origin, operator_token).await
}

pub async fn exchange_subject_token_at(
    client: &reqwest::Client,
    origin: &str,
    operator_token: &str,
) -> SyncResult<String> {
    let url = format!("{origin}/api/v1/core/oauth/token");
    let response = client
        .post(&url)
        .form(&[
            ("grant_type", TOKEN_EXCHANGE_GRANT),
            ("subject_token", operator_token),
            ("subject_token_type", SUBJECT_TOKEN_TYPE_JWT),
        ])
        .send()
        .await?;

    let parsed: TokenExchangeResponse = response::handle_json(response).await?;
    Ok(parsed.access_token)
}
