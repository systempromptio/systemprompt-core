//! Signed-manifest fetch and public-key resolution for plugin sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::auth::secret::Secret;
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::{SignedManifest, SignedManifestVerify};
use crate::ids::PinnedPubKey;

fn map_gateway_error(err: GatewayError, endpoint: &'static str) -> SyncError {
    match err {
        GatewayError::HttpStatus { status, .. } if matches!(status.as_u16(), 401 | 403) => {
            SyncError::GatewayUnauthorized {
                bin: crate::brand::brand().binary_name,
                endpoint,
                status: status.as_u16(),
            }
        },
        other => SyncError::Network(other.to_string()),
    }
}

pub(super) struct ManifestFetch {
    pub client: GatewayClient,
    pub bearer: Secret,
    pub manifest: SignedManifest,
}

pub(super) async fn fetch_authenticated_manifest() -> Result<ManifestFetch, SyncError> {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);

    let bearer = match crate::auth::cache::read_valid() {
        Some(out) => out.token,
        None => fetch_fresh_token().await.ok_or(SyncError::NoCredential {
            bin: crate::brand::brand().binary_name,
        })?,
    };

    let client = GatewayClient::new(gateway);
    let manifest = client
        .fetch_manifest(bearer.expose())
        .await
        .map_err(|e| map_gateway_error(e, "manifest"))?;

    Ok(ManifestFetch {
        client,
        bearer,
        manifest,
    })
}

pub(super) async fn verify_signature(
    fetch: &ManifestFetch,
    allow_unsigned: bool,
    allow_tofu: bool,
) -> Result<(), SyncError> {
    if allow_unsigned {
        return Ok(());
    }
    let pubkey = resolve_pubkey(&fetch.client, allow_tofu).await?;
    fetch
        .manifest
        .verify(pubkey.as_str())
        .map_err(|e| SyncError::SignatureFailed(e.to_string()))
}

async fn resolve_pubkey(
    client: &GatewayClient,
    allow_tofu: bool,
) -> Result<PinnedPubKey, SyncError> {
    if let Some(k) = config::pinned_pubkey() {
        return Ok(k);
    }
    if !allow_tofu {
        return Err(SyncError::PubkeyNotPinned);
    }
    tracing::info!("first-run trust-on-first-use: fetching manifest pubkey from gateway");
    let fetched = client
        .fetch_pubkey()
        .await
        .map_err(|e| map_gateway_error(e, "pubkey"))?;
    if let Err(e) = config::persist_pinned_pubkey(&fetched) {
        tracing::warn!(error = %e, "failed to persist pinned pubkey; next run will re-trust on first use");
    }
    let prefix: String = fetched.chars().take(12).collect();
    tracing::info!(
        "pinned manifest pubkey ({prefix}…) — future syncs will reject any pubkey rotation"
    );
    Ok(PinnedPubKey::new(fetched))
}

async fn fetch_fresh_token() -> Option<Secret> {
    use crate::auth::providers::AuthError;
    use systemprompt_identifiers::SessionId;
    let cfg = config::load();
    let session_id = SessionId::generate();
    let chain = crate::auth::provider_chain(&cfg);
    let mut not_configured: Vec<&'static str> = Vec::new();
    let mut had_failure = false;
    for p in &chain {
        match p.authenticate(&session_id).await {
            Ok(out) => {
                if let Err(e) = crate::auth::cache::write(&out) {
                    tracing::warn!(error = %e, "failed to cache fresh token; will re-authenticate next call");
                }
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => {
                not_configured.push(p.name());
            },
            Err(e @ AuthError::Failed { .. }) => {
                had_failure = true;
                crate::obs::output::diag(&format!("{}: {e}", p.name()));
            },
        }
    }
    if !had_failure {
        let tried = not_configured.join(", ");
        let bin = crate::brand::brand().binary_name;
        tracing::warn!(
            providers = %tried,
            bin = %bin,
            "no auth provider is configured; run login to register a PAT before syncing",
        );
        crate::obs::output::diag(&format!(
            "no auth provider configured (tried: {tried}); run `{bin} login <sp-live-...>`"
        ));
    }
    None
}
