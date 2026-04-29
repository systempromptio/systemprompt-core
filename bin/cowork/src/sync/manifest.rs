use super::error::SyncError;
use crate::auth::secret::Secret;
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::manifest::SignedManifest;
use crate::ids::PinnedPubKey;

pub struct ManifestFetch {
    pub client: GatewayClient,
    pub bearer: Secret,
    pub manifest: SignedManifest,
}

pub fn fetch_authenticated_manifest() -> Result<ManifestFetch, SyncError> {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);

    let bearer = match crate::auth::cache::read_valid() {
        Some(out) => out.token,
        None => fetch_fresh_token().ok_or(SyncError::NoCredential)?,
    };

    let client = GatewayClient::new(gateway);
    let manifest = client
        .fetch_manifest(bearer.expose())
        .map_err(|e| SyncError::Network(e.to_string()))?;

    Ok(ManifestFetch {
        client,
        bearer,
        manifest,
    })
}

pub fn verify_signature(
    fetch: &ManifestFetch,
    allow_unsigned: bool,
    allow_tofu: bool,
) -> Result<(), SyncError> {
    if allow_unsigned {
        return Ok(());
    }
    let pubkey = resolve_pubkey(&fetch.client, allow_tofu)?;
    fetch
        .manifest
        .verify(pubkey.as_str())
        .map_err(|e| SyncError::SignatureFailed(e.to_string()))
}

fn resolve_pubkey(client: &GatewayClient, allow_tofu: bool) -> Result<PinnedPubKey, SyncError> {
    if let Some(k) = config::pinned_pubkey() {
        return Ok(k);
    }
    if !allow_tofu {
        return Err(SyncError::PubkeyNotPinned);
    }
    tracing::info!("first-run trust-on-first-use: fetching manifest pubkey from gateway");
    let fetched = client
        .fetch_pubkey()
        .map_err(|e| SyncError::Network(e.to_string()))?;
    let _ = config::persist_pinned_pubkey(&fetched);
    let prefix: String = fetched.chars().take(12).collect();
    tracing::info!(
        "pinned manifest pubkey ({prefix}…) — future syncs will reject any pubkey rotation"
    );
    Ok(PinnedPubKey::new(fetched))
}

fn fetch_fresh_token() -> Option<Secret> {
    use crate::auth::providers::{AuthError, AuthProvider};
    let cfg = config::load();
    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(crate::auth::providers::mtls::MtlsProvider::new(&cfg)),
        Box::new(crate::auth::providers::session::SessionProvider::new(&cfg)),
        Box::new(crate::auth::providers::pat::PatProvider::new(&cfg)),
    ];
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = crate::auth::cache::write(&out);
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => {},
            Err(e @ AuthError::Failed { .. }) => {
                crate::obs::output::diag(&format!("{}: {e}", p.name()));
            },
        }
    }
    None
}
