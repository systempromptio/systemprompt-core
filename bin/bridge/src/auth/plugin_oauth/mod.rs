//! Plugin-scoped OAuth hook tokens.
//!
//! `client_secret` lives in the OS keystore (Keychain / Credential Manager /
//! Secret Service); non-secret fields in a 0600 JSON file under the cache dir.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod secret_store;
mod token;

pub use secret_store::{SecretBackend, credential_backend};
pub use token::{
    CachedHookToken, PluginTokenCache, REFRESH_THRESHOLD_SECS, mint_or_refresh_plugin_token,
};

use crate::gateway::{BridgeOAuthClientResponse, GatewayClient, GatewayError};
use crate::ids::BearerToken;
use secret_store::{delete_secret, read_secret, write_secret};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{fs, io};
use systemprompt_identifiers::ClientId;

const CREDS_FILE: &str = "oauth_client.json";

#[derive(Debug, thiserror::Error)]
pub enum PluginOAuthError {
    #[error("OAuth client credentials path is unresolvable")]
    CredsPathUnresolvable,
    #[error("read OAuth client credentials: {0}")]
    CredsRead(#[source] io::Error),
    #[error("write OAuth client credentials: {0}")]
    CredsWrite(#[source] io::Error),
    #[error("decode OAuth client credentials: {0}")]
    CredsDecode(#[from] serde_json::Error),
    #[error("keyring: {0}")]
    Keyring(#[source] keyring_core::Error),
    #[error("no usable credential store: secret-service ({secret_service}), keyutils ({keyutils})")]
    NoCredentialStore {
        secret_service: String,
        #[source]
        keyutils: keyring_core::Error,
    },
    #[error("in-memory secret store lock was poisoned")]
    MemoryStorePoisoned,
    #[error("remove OAuth client credentials: {0}")]
    CredsRemove(#[source] io::Error),
    #[error(transparent)]
    Trust(#[from] crate::config::TrustError),
    #[error("gateway: {0}")]
    Gateway(#[from] GatewayError),
}

#[derive(Debug, Clone)]
pub struct OAuthClientCreds {
    pub client_id: ClientId,
    pub client_secret: String,
    pub token_endpoint: String,
    pub scopes: Vec<String>,
    pub gateway: Option<String>,
}

impl From<BridgeOAuthClientResponse> for OAuthClientCreds {
    fn from(r: BridgeOAuthClientResponse) -> Self {
        Self {
            client_id: r.client_id,
            client_secret: r.client_secret,
            token_endpoint: r.token_endpoint,
            scopes: r.scopes,
            gateway: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredCreds {
    client_id: ClientId,
    token_endpoint: String,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gateway: Option<String>,
}

pub fn creds_path() -> Option<PathBuf> {
    let base = crate::basedirs::cache_dir()?;
    Some(
        base.join(crate::brand::brand().working_dir_name)
            .join(CREDS_FILE),
    )
}

pub fn store_creds(creds: &OAuthClientCreds) -> Result<(), PluginOAuthError> {
    let path = creds_path().ok_or(PluginOAuthError::CredsPathUnresolvable)?;
    let stored = StoredCreds {
        client_id: creds.client_id.clone(),
        token_endpoint: creds.token_endpoint.clone(),
        scopes: creds.scopes.clone(),
        gateway: creds.gateway.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&stored)?;
    // Why: the secret goes to the keystore before the metadata names it; a
    // keystore failure then leaves the previous, still-usable pair in place.
    write_secret(&creds.client_id, &creds.client_secret)?;
    crate::fsutil::atomic_write_0600(&path, &bytes).map_err(PluginOAuthError::CredsWrite)?;
    Ok(())
}

pub fn load_creds() -> Result<Option<OAuthClientCreds>, PluginOAuthError> {
    let Some(path) = creds_path() else {
        return Err(PluginOAuthError::CredsPathUnresolvable);
    };
    let Some(text) = crate::fsutil::read_optional(&path).map_err(PluginOAuthError::CredsRead)?
    else {
        return Ok(None);
    };
    let stored: StoredCreds = serde_json::from_str(&text)?;
    let Some(secret) = read_secret(&stored.client_id)? else {
        tracing::warn!(client_id = %stored.client_id, "OAuth metadata on disk but no keyring entry; treating as unprovisioned");
        return Ok(None);
    };
    Ok(Some(OAuthClientCreds {
        client_id: stored.client_id,
        client_secret: secret,
        token_endpoint: stored.token_endpoint,
        scopes: stored.scopes,
        gateway: stored.gateway,
    }))
}

pub fn delete_creds() -> Result<(), PluginOAuthError> {
    let Some(path) = creds_path() else {
        return Ok(());
    };
    if let Some(text) = crate::fsutil::read_optional(&path).map_err(PluginOAuthError::CredsRead)? {
        let stored: StoredCreds = serde_json::from_str(&text)?;
        delete_secret(&stored.client_id)?;
    }
    match fs::remove_file(&path) {
        Ok(()) | Err(_) if !path.exists() => Ok(()),
        Ok(()) => Ok(()),
        Err(e) => Err(PluginOAuthError::CredsRemove(e)),
    }
}

fn load_creds_for(
    gateway: &systemprompt_identifiers::ValidatedUrl,
) -> Result<Option<OAuthClientCreds>, PluginOAuthError> {
    let Some(existing) = load_creds()? else {
        return Ok(None);
    };
    let current = crate::config::trust::GatewayIdentity::new(gateway)?;
    let stored = existing
        .gateway
        .as_deref()
        .map(crate::config::trust::GatewayIdentity::parse)
        .transpose()?;
    if stored.as_ref() == Some(&current) {
        return Ok(Some(existing));
    }
    tracing::info!(
        target: "bridge::auth::plugin-oauth",
        client_id = %existing.client_id,
        stored_gateway = existing.gateway.as_deref().unwrap_or("<unrecorded>"),
        gateway = %current,
        "stored OAuth client belongs to a different gateway; re-provisioning"
    );
    Ok(None)
}

pub async fn ensure_creds(
    gateway: &GatewayClient,
    bearer: &BearerToken,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    if let Some(existing) = load_creds_for(gateway.base_url())? {
        return Ok(existing);
    }
    provision(gateway, bearer).await
}

pub async fn refresh_creds(
    gateway: &GatewayClient,
    bearer: &BearerToken,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    provision(gateway, bearer).await
}

async fn provision(
    gateway: &GatewayClient,
    bearer: &BearerToken,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    let response = gateway.provision_oauth_client(bearer).await?;
    let mut creds: OAuthClientCreds = response.into();
    creds.gateway = Some(gateway.base_url_str().to_owned());
    store_creds(&creds)?;
    Ok(creds)
}
