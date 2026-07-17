//! Plugin-scoped OAuth hook tokens.
//!
//! `client_secret` lives in the OS keystore (Keychain / Credential Manager /
//! Secret Service); non-secret fields in a 0600 JSON file under the cache dir.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::{BridgeOAuthClientResponse, GatewayClient, GatewayError, HookTokenResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};
use systemprompt_identifiers::{ClientId, PluginId};
use tokio::sync::OnceCell;

pub const REFRESH_THRESHOLD_SECS: u64 = 300;

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
    Keyring(String),
    #[error("gateway: {0}")]
    Gateway(#[from] GatewayError),
}

#[derive(Debug, Clone)]
pub struct OAuthClientCreds {
    pub client_id: ClientId,
    pub client_secret: String,
    pub token_endpoint: String,
    pub scopes: Vec<String>,
}

impl From<BridgeOAuthClientResponse> for OAuthClientCreds {
    fn from(r: BridgeOAuthClientResponse) -> Self {
        Self {
            client_id: r.client_id,
            client_secret: r.client_secret,
            token_endpoint: r.token_endpoint,
            scopes: r.scopes,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredCreds {
    client_id: ClientId,
    token_endpoint: String,
    #[serde(default)]
    scopes: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LegacyCreds {
    client_id: ClientId,
    client_secret: String,
    token_endpoint: String,
    #[serde(default)]
    scopes: Vec<String>,
}

pub fn creds_path() -> Option<PathBuf> {
    let base = dirs::cache_dir()?;
    Some(
        base.join(crate::brand::brand().working_dir_name)
            .join(CREDS_FILE),
    )
}

fn keyring_entry(client_id: &ClientId) -> Result<keyring::Entry, PluginOAuthError> {
    keyring::Entry::new(crate::brand::brand().keyring_service, client_id.as_str())
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()))
}

fn write_secret(client_id: &ClientId, secret: &str) -> Result<(), PluginOAuthError> {
    keyring_entry(client_id)?
        .set_password(secret)
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()))
}

fn read_secret(client_id: &ClientId) -> Result<Option<String>, PluginOAuthError> {
    match keyring_entry(client_id)?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
    }
}

fn delete_secret(client_id: &ClientId) {
    match keyring_entry(client_id).and_then(|e| match e.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
    }) {
        Ok(()) => {},
        Err(e) => tracing::warn!(error = %e, "keyring delete failed"),
    }
}

pub fn store_creds(creds: &OAuthClientCreds) -> Result<(), PluginOAuthError> {
    let path = creds_path().ok_or(PluginOAuthError::CredsPathUnresolvable)?;
    let stored = StoredCreds {
        client_id: creds.client_id.clone(),
        token_endpoint: creds.token_endpoint.clone(),
        scopes: creds.scopes.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&stored)?;
    crate::fsutil::atomic_write_0600(&path, &bytes).map_err(PluginOAuthError::CredsWrite)?;
    write_secret(&creds.client_id, &creds.client_secret)?;
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
    let raw: serde_json::Value = serde_json::from_str(&text)?;
    if raw.get("client_secret").is_some() {
        let l: LegacyCreds = serde_json::from_value(raw)?;
        tracing::info!(client_id = %l.client_id, "migrating legacy OAuth client_secret into OS keystore");
        let creds = OAuthClientCreds {
            client_id: l.client_id,
            client_secret: l.client_secret,
            token_endpoint: l.token_endpoint,
            scopes: l.scopes,
        };
        store_creds(&creds)?;
        return Ok(Some(creds));
    }
    let stored: StoredCreds = serde_json::from_value(raw)?;
    let Some(secret) = read_secret(&stored.client_id)? else {
        tracing::warn!(client_id = %stored.client_id, "OAuth metadata on disk but no keyring entry; treating as unprovisioned");
        return Ok(None);
    };
    Ok(Some(OAuthClientCreds {
        client_id: stored.client_id,
        client_secret: secret,
        token_endpoint: stored.token_endpoint,
        scopes: stored.scopes,
    }))
}

pub fn delete_creds() -> io::Result<()> {
    let Some(path) = creds_path() else {
        return Ok(());
    };
    if let Some(text) = crate::fsutil::read_optional(&path)?
        && let Ok(stored) = serde_json::from_str::<StoredCreds>(&text)
    {
        delete_secret(&stored.client_id);
    }
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

// Provisioning rotates the per-tenant secret server-side; only call when local
// state is missing.
pub async fn ensure_creds(
    gateway: &GatewayClient,
    pat: &str,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    if let Some(existing) = load_creds()? {
        return Ok(existing);
    }
    let response = gateway.provision_oauth_client(pat).await?;
    let creds: OAuthClientCreds = response.into();
    store_creds(&creds)?;
    Ok(creds)
}

pub async fn refresh_creds(
    gateway: &GatewayClient,
    pat: &str,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    let response = gateway.provision_oauth_client(pat).await?;
    let creds: OAuthClientCreds = response.into();
    store_creds(&creds)?;
    Ok(creds)
}

#[derive(Debug, Clone)]
pub struct CachedHookToken {
    pub access_token: String,
    pub expires_at_unix: u64,
}

impl CachedHookToken {
    fn from_response(r: HookTokenResponse) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let lifetime = u64::try_from(r.expires_in.max(0)).unwrap_or(0);
        Self {
            access_token: r.access_token,
            expires_at_unix: now.saturating_add(lifetime),
        }
    }

    fn is_fresh(&self, threshold_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.expires_at_unix > now.saturating_add(threshold_secs)
    }
}

#[derive(Debug, Default)]
pub struct PluginTokenCache {
    entries: Mutex<HashMap<String, CachedHookToken>>,
}

impl PluginTokenCache {
    pub fn get(&self, plugin_id: &PluginId, threshold_secs: u64) -> Option<CachedHookToken> {
        let guard = self.entries.lock().ok()?;
        let cached = guard.get(plugin_id.as_str())?.clone();
        drop(guard);
        cached.is_fresh(threshold_secs).then_some(cached)
    }

    pub fn put(&self, plugin_id: &PluginId, token: CachedHookToken) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(plugin_id.as_str().to_owned(), token);
        }
    }

    pub fn invalidate(&self, plugin_id: &PluginId) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.remove(plugin_id.as_str());
        }
    }
}

static GLOBAL_CACHE: OnceCell<PluginTokenCache> = OnceCell::const_new();

pub async fn global_cache() -> &'static PluginTokenCache {
    GLOBAL_CACHE
        .get_or_init(|| async { PluginTokenCache::default() })
        .await
}

async fn mint(
    gateway: &GatewayClient,
    c: &OAuthClientCreds,
    plugin_id: &PluginId,
) -> Result<HookTokenResponse, GatewayError> {
    gateway
        .mint_plugin_hook_token(&c.token_endpoint, &c.client_id, &c.client_secret, plugin_id)
        .await
}

pub async fn mint_or_refresh_plugin_token(
    gateway: &GatewayClient,
    pat: &str,
    plugin_id: &PluginId,
) -> Result<CachedHookToken, PluginOAuthError> {
    let cache = global_cache().await;
    if let Some(cached) = cache.get(plugin_id, REFRESH_THRESHOLD_SECS) {
        return Ok(cached);
    }
    let creds = ensure_creds(gateway, pat).await?;
    let response = match mint(gateway, &creds, plugin_id).await {
        Ok(r) => r,
        Err(GatewayError::HookTokenRejected { status, .. }) if status.as_u16() == 401 => {
            tracing::warn!(
                plugin_id = plugin_id.as_str(),
                "hook token mint 401; rotating client secret and retrying"
            );
            let creds = refresh_creds(gateway, pat).await?;
            mint(gateway, &creds, plugin_id).await?
        },
        Err(e) => return Err(e.into()),
    };
    let cached = CachedHookToken::from_response(response);
    cache.put(plugin_id, cached.clone());
    Ok(cached)
}
