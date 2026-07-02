//! Per-user bridge hook OAuth client provisioning (client-credentials grant).

use serde::Serialize;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, UserId};

use crate::error::OauthResult as Result;
use crate::repository::{CreateClientParams, OAuthRepository};
use crate::services::generation::{generate_client_secret, hash_client_secret};

const BRIDGE_HOOK_CLIENT_SCOPES: &[&str] = &["hook:govern", "hook:track"];

fn bridge_hook_client_id(user_id: &UserId) -> ClientId {
    ClientId::new(format!("bridge:{}", user_id.as_str()))
}

/// Plaintext `client_secret` is returned only at creation/rotation time; the
/// database stores only the bcrypt hash. The bridge MUST persist this secret
/// on receipt — the server cannot re-emit it.
#[derive(Debug, Clone, Serialize)]
pub struct BridgeOAuthClient {
    pub client_id: ClientId,
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub token_endpoint: String,
}

pub async fn provision_bridge_oauth_client(
    pool: &DbPool,
    user_id: &UserId,
    token_endpoint: String,
) -> Result<BridgeOAuthClient> {
    let repo = OAuthRepository::new(pool)?;
    let client_id = bridge_hook_client_id(user_id);
    let secret = generate_client_secret();
    let secret_hash = hash_client_secret(&secret)?;

    let scopes: Vec<String> = BRIDGE_HOOK_CLIENT_SCOPES
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let existing = repo.find_client_by_id(&client_id).await?;
    if existing.is_some() {
        repo.update_client_secret(&client_id, &secret_hash).await?;
    } else {
        let params = CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: user_id.clone(),
            client_secret_hash: secret_hash,
            client_name: format!("bridge hook client for {}", user_id.as_str()),
            redirect_uris: Vec::new(),
            grant_types: Some(vec!["client_credentials".to_owned()]),
            response_types: Some(Vec::new()),
            scopes: scopes.clone(),
            token_endpoint_auth_method: Some("client_secret_post".to_owned()),
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        };
        repo.create_client(params).await?;
    }

    Ok(BridgeOAuthClient {
        client_id,
        client_secret: secret,
        scopes,
        token_endpoint,
    })
}
