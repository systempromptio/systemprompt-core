//! Provisioning of the `sys_sync` confidential OAuth client.
//!
//! The deployment's `SYNC_TOKEN` secret is registered as the `client_secret`
//! of a `client_credentials` OAuth client. The sync caller exchanges that
//! secret for a short-lived `Service`-type JWT and calls `/api/v1/sync/*`
//! with the JWT — `/sync` is governed by the standard authz framework.

use crate::error::OauthResult as Result;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ClientId;

use crate::repository::{CreateClientParams, OAuthRepository};
use crate::services::generation::hash_client_secret;

const SYNC_CLIENT_SCOPES: &[&str] = &["service"];

/// Idempotent: re-running with a different `sync_token` rotates the stored
/// `client_secret_hash`, so editing `SYNC_TOKEN` and rebooting is the
/// supported rotation path. `sync_token` is reused verbatim as the OAuth
/// `client_secret`; the database stores only its hash.
pub async fn provision_sync_oauth_client(pool: &DbPool, sync_token: &str) -> Result<()> {
    let repo = OAuthRepository::new(pool)?;
    let client_id = ClientId::sync();
    let secret_hash = hash_client_secret(sync_token)?;

    let scopes: Vec<String> = SYNC_CLIENT_SCOPES
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    if repo.find_client_by_id(&client_id).await?.is_some() {
        repo.update_client_secret(&client_id, &secret_hash).await?;
    } else {
        let params = CreateClientParams {
            client_id: client_id.clone(),
            client_secret_hash: secret_hash,
            client_name: "systemprompt cloud sync client".to_string(),
            redirect_uris: Vec::new(),
            grant_types: Some(vec!["client_credentials".to_string()]),
            response_types: Some(Vec::new()),
            scopes,
            token_endpoint_auth_method: Some("client_secret_post".to_string()),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        };
        repo.create_client(params).await?;
    }

    Ok(())
}
