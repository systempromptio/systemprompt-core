//! Plugin-scoped OAuth hook tokens.
//!
//! `client_secret` lives in the OS keystore (Keychain / Credential Manager /
//! Secret Service); non-secret fields in a 0600 JSON file under the cache dir.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::{BridgeOAuthClientResponse, GatewayClient, GatewayError, HookTokenResponse};
use crate::ids::BearerToken;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, OnceLock};
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
    let base = crate::basedirs::cache_dir()?;
    Some(
        base.join(crate::brand::brand().working_dir_name)
            .join(CREDS_FILE),
    )
}

/// Which backend `write_secret`/`read_secret`/`delete_secret` are using.
///
/// Resolved exactly once, because `keyring_core`'s default store is process
/// global and set-once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretBackend {
    Keyring,
    Memory,
}

impl SecretBackend {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Keyring => "keyring",
            Self::Memory => "memory",
        }
    }
}

static BACKEND: OnceLock<SecretBackend> = OnceLock::new();
static MEMORY_SECRETS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The backend in use, for `doctor` to report. Resolves the store on first
/// call.
pub fn credential_backend() -> SecretBackend {
    resolve_backend()
}

fn resolve_backend() -> SecretBackend {
    if let Some(known) = BACKEND.get() {
        return *known;
    }
    let backend = match install_store() {
        Ok(()) => SecretBackend::Keyring,
        Err(e) => {
            tracing::warn!(
                target: "bridge::auth::keystore",
                error = %e,
                "no OS credential store available; holding the OAuth client secret in memory for \
                 this process only. It is re-provisioned from the gateway on restart, so hooks \
                 keep working, but a second process (e.g. `doctor`) will report no client. \
                 Install a Secret Service provider (gnome-keyring), or allow kernel keyrings \
                 (docker: --security-opt seccomp=unconfined), to make it persistent."
            );
            SecretBackend::Memory
        },
    };
    *BACKEND.get_or_init(|| backend)
}

/// Installs a platform credential store, but only if the process has none.
///
/// The guard is what lets the bridge test suites pre-install a headless
/// keyutils store: an unconditional registration would clobber it on the first
/// entry.
fn install_store() -> Result<(), PluginOAuthError> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()));
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new()
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()));
    #[cfg(all(unix, not(target_os = "macos")))]
    let store = linux_store();

    let store = store?;
    keyring_core::set_default_store(store);
    Ok(())
}

/// Secret Service first, then the kernel keyutils keyring.
///
/// A headless Linux box — a server, a container, CI — has no Secret Service
/// provider, so the D-Bus store fails to construct even when a session bus is
/// present. Without a fallback the first plugin hook request dies at the token
/// mint and the client only sees a bare 502. Keyutils keeps the secret in the
/// kernel keyring instead: not persistent across a reboot, which is acceptable
/// because the secret is re-mintable and losing it costs one re-provision.
///
/// Construction is not proof of usability: Docker's default seccomp profile
/// denies `add_key`/`keyctl`, so the store builds and then fails `EPERM` on the
/// first write. A round-trip probe is what keeps that from becoming the same
/// 502 in a new place.
#[cfg(all(unix, not(target_os = "macos")))]
fn linux_store() -> Result<std::sync::Arc<keyring_core::CredentialStore>, PluginOAuthError> {
    let dbus_err = match dbus_secret_service_keyring_store::Store::new() {
        Ok(store) => return Ok(store),
        Err(e) => e.to_string(),
    };
    let store: std::sync::Arc<keyring_core::CredentialStore> =
        linux_keyutils_keyring_store::Store::new().map_err(|e| {
            PluginOAuthError::Keyring(format!(
                "no usable credential store: secret-service ({dbus_err}), keyutils ({e})"
            ))
        })?;
    probe_store(&store).map_err(|e| {
        PluginOAuthError::Keyring(format!(
            "no usable credential store: secret-service ({dbus_err}), \
             keyutils built but is unusable ({e})"
        ))
    })?;
    tracing::warn!(
        target: "bridge::auth::keystore",
        secret_service_error = %dbus_err,
        "no Secret Service provider on this host (headless Linux?); using the kernel keyutils \
         keyring. The OAuth client secret will not survive a reboot and is re-provisioned \
         automatically."
    );
    Ok(store)
}

/// Write/read/delete a throwaway entry to prove the store actually works.
#[cfg(all(unix, not(target_os = "macos")))]
fn probe_store(store: &std::sync::Arc<keyring_core::CredentialStore>) -> Result<(), String> {
    let service = format!("{}-probe", crate::brand::brand().keyring_service);
    let entry = store
        .build(&service, "probe", None)
        .map_err(|e| format!("build probe entry: {e}"))?;
    entry
        .set_password("probe")
        .map_err(|e| format!("probe write: {e}"))?;
    entry
        .get_password()
        .map_err(|e| format!("probe read: {e}"))?;
    if let Err(e) = entry.delete_credential() {
        tracing::debug!(
            target: "bridge::auth::keystore",
            error = %e,
            "credential-store probe entry could not be removed; it is overwritten on each probe"
        );
    }
    Ok(())
}

fn keyring_entry(client_id: &ClientId) -> Result<keyring_core::Entry, PluginOAuthError> {
    keyring_core::Entry::new(crate::brand::brand().keyring_service, client_id.as_str())
        .map_err(|e| PluginOAuthError::Keyring(e.to_string()))
}

fn write_secret(client_id: &ClientId, secret: &str) -> Result<(), PluginOAuthError> {
    match resolve_backend() {
        SecretBackend::Keyring => keyring_entry(client_id)?
            .set_password(secret)
            .map_err(|e| PluginOAuthError::Keyring(e.to_string())),
        SecretBackend::Memory => {
            memory_secrets()?.insert(client_id.as_str().to_owned(), secret.to_owned());
            Ok(())
        },
    }
}

fn read_secret(client_id: &ClientId) -> Result<Option<String>, PluginOAuthError> {
    match resolve_backend() {
        SecretBackend::Keyring => match keyring_entry(client_id)?.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
        },
        SecretBackend::Memory => Ok(memory_secrets()?.get(client_id.as_str()).cloned()),
    }
}

fn delete_secret(client_id: &ClientId) {
    let outcome = match resolve_backend() {
        SecretBackend::Keyring => {
            keyring_entry(client_id).and_then(|e| match e.delete_credential() {
                Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
                Err(e) => Err(PluginOAuthError::Keyring(e.to_string())),
            })
        },
        SecretBackend::Memory => memory_secrets().map(|mut m| {
            m.remove(client_id.as_str());
        }),
    };
    if let Err(e) = outcome {
        tracing::warn!(
            target: "bridge::auth::keystore",
            backend = resolve_backend().as_str(),
            error = %e,
            "could not delete the stored OAuth client secret; it will be overwritten on the next \
             provision"
        );
    }
}

fn memory_secrets()
-> Result<std::sync::MutexGuard<'static, HashMap<String, String>>, PluginOAuthError> {
    MEMORY_SECRETS.lock().map_err(|_poisoned| {
        PluginOAuthError::Keyring("in-memory secret store lock was poisoned".to_owned())
    })
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
    bearer: &BearerToken,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    if let Some(existing) = load_creds()? {
        return Ok(existing);
    }
    let response = gateway.provision_oauth_client(bearer).await?;
    let creds: OAuthClientCreds = response.into();
    store_creds(&creds)?;
    Ok(creds)
}

pub async fn refresh_creds(
    gateway: &GatewayClient,
    bearer: &BearerToken,
) -> Result<OAuthClientCreds, PluginOAuthError> {
    let response = gateway.provision_oauth_client(bearer).await?;
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
    let endpoint = gateway_aligned_endpoint(&c.token_endpoint, gateway.base_url_str());
    gateway
        .mint_plugin_hook_token(&endpoint, &c.client_id, &c.client_secret, plugin_id)
        .await
}

/// Re-points a loopback `token_endpoint` at the gateway actually being dialled.
///
/// The gateway advertises its own absolute token endpoint, which is
/// `http://localhost:8080/...` whenever it is itself reached over loopback. A
/// client on another host — a container, another machine — cannot resolve that
/// to the gateway, so the hook-token mint fails with a bare connection error
/// and every plugin hook 503s. Mirrors `sync::apply::rewrite_loopback_urls`,
/// which solves the identical problem for managed MCP server URLs.
fn gateway_aligned_endpoint(raw: &str, gateway: &str) -> String {
    let (Ok(mut parsed), Ok(gw)) = (url::Url::parse(raw), url::Url::parse(gateway)) else {
        return raw.to_owned();
    };
    let is_loopback = match parsed.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(a)) => a.is_loopback(),
        Some(url::Host::Ipv6(a)) => a.is_loopback(),
        None => false,
    };
    if !is_loopback {
        return raw.to_owned();
    }
    let Some(gw_host) = gw.host_str() else {
        return raw.to_owned();
    };
    if parsed.set_scheme(gw.scheme()).is_err()
        || parsed.set_host(Some(gw_host)).is_err()
        || parsed.set_port(gw.port()).is_err()
    {
        return raw.to_owned();
    }
    let rebuilt = parsed.to_string();
    tracing::info!(
        target: "bridge::auth::plugin-oauth",
        original = %raw,
        rewritten = %rebuilt,
        "gateway advertised a loopback hook-token endpoint; re-pointed it at the configured gateway"
    );
    rebuilt
}

/// `bearer` is whatever credential the proxy authenticated the caller with —
/// in practice the bridge JWT, not a PAT.
pub async fn mint_or_refresh_plugin_token(
    gateway: &GatewayClient,
    bearer: &BearerToken,
    plugin_id: &PluginId,
) -> Result<CachedHookToken, PluginOAuthError> {
    let cache = global_cache().await;
    if let Some(cached) = cache.get(plugin_id, REFRESH_THRESHOLD_SECS) {
        return Ok(cached);
    }
    let creds = ensure_creds(gateway, bearer).await?;
    let response = match mint(gateway, &creds, plugin_id).await {
        Ok(r) => r,
        Err(GatewayError::HookTokenRejected { status, .. }) if status.as_u16() == 401 => {
            tracing::warn!(
                plugin_id = plugin_id.as_str(),
                "hook token mint 401; rotating client secret and retrying"
            );
            let creds = refresh_creds(gateway, bearer).await?;
            mint(gateway, &creds, plugin_id).await?
        },
        Err(e) => return Err(e.into()),
    };
    let cached = CachedHookToken::from_response(response);
    cache.put(plugin_id, cached.clone());
    Ok(cached)
}
