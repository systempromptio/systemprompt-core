//! Signed-manifest fetch and public-key resolution for plugin sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::auth::secret::Secret;
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::{
    ManifestError, SignedManifest, SignedManifestEnvelope, decode_payload, verify_envelope,
};
use crate::ids::PinnedPubKey;

struct RejectedCredential<'a> {
    credential: &'static str,
    token: Option<&'a Secret>,
}

fn map_gateway_error(
    err: GatewayError,
    endpoint: &'static str,
    rejected: &RejectedCredential<'_>,
) -> SyncError {
    match err {
        GatewayError::HttpStatus { status, .. } if matches!(status.as_u16(), 401 | 403) => {
            unauthorized(endpoint, status.as_u16(), rejected)
        },
        GatewayError::ManifestDecode(e) if e.is_decode() => SyncError::ManifestShape(e.to_string()),
        e @ GatewayError::ManifestEnvelopeShape { .. } => SyncError::ManifestShape(e.to_string()),
        other => SyncError::Network(other.to_string()),
    }
}

fn unauthorized(
    endpoint: &'static str,
    status: u16,
    rejected: &RejectedCredential<'_>,
) -> SyncError {
    let cfg = config::load();
    let identity = rejected
        .token
        .and_then(|t| crate::auth::jwt::decode_unverified(t.expose()))
        .and_then(|c| c.display_label())
        .map(|label| format!(" for {label}"))
        .unwrap_or_default();
    let (config_file, pat_file) = match crate::auth::setup::resolve_paths() {
        Ok(p) => (
            p.config_file.display().to_string(),
            p.pat_file.display().to_string(),
        ),
        Err(_) => ("<unresolvable>".to_owned(), "<unresolvable>".to_owned()),
    };
    SyncError::GatewayUnauthorized(Box::new(super::error::CredentialRejection {
        bin: crate::brand::brand().binary_name,
        endpoint,
        status,
        gateway: config::gateway_url_or_default(&cfg).to_string(),
        credential: rejected.credential,
        identity,
        config_file,
        pat_file,
        override_note: credential_dir_override_note(),
    }))
}

fn credential_dir_override_note() -> String {
    let mut overrides = Vec::new();
    let config_env = crate::brand::brand().env("CONFIG");
    if std::env::var_os(&config_env).is_some() {
        overrides.push(config_env);
    }
    if crate::basedirs::config_home_override().is_some() {
        overrides.push("XDG_CONFIG_HOME".to_owned());
    }
    if overrides.is_empty() {
        String::new()
    } else {
        format!(
            " — note the credential location for this process is redirected by {}; a bridge \
             launched from the desktop resolves the default location instead",
            overrides.join(" and ")
        )
    }
}

fn map_manifest_error(err: ManifestError) -> SyncError {
    match err {
        ManifestError::SchemaTooNew {
            required,
            supported,
        } => SyncError::SchemaTooNew {
            required,
            supported,
        },
        ManifestError::BridgeTooOld { local, required } => {
            SyncError::BridgeTooOld { local, required }
        },
        ManifestError::PayloadParse(e) => SyncError::ManifestShape(e.to_string()),
        other => SyncError::SignatureFailed(other.to_string()),
    }
}

pub(super) struct ManifestFetch {
    pub client: GatewayClient,
    pub bearer: Secret,
    pub envelope: SignedManifestEnvelope,
}

pub(super) async fn fetch_authenticated_manifest(
    http: &reqwest::Client,
) -> Result<ManifestFetch, SyncError> {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway.clone(), http.clone());

    let no_credential = || SyncError::NoCredential {
        bin: crate::brand::brand().binary_name,
    };

    let cached = crate::auth::cache::read_valid(&gateway).map(|out| out.token);
    let was_cached = cached.is_some();
    let mut bearer = match cached {
        Some(token) => token,
        None => fetch_fresh_token(http).await.ok_or_else(no_credential)?,
    };

    let mut envelope = client.fetch_manifest(bearer.expose()).await;

    // Why: a rejected cached token outlives every re-login until its TTL
    // lapses, wedging the install permanently; dropping it and minting once
    // is the only way out that does not require the user to run `logout`.
    if is_unauthorized(&envelope) && was_cached {
        tracing::warn!("gateway refused the cached token; discarding it and re-authenticating");
        if let Err(e) = crate::auth::cache::clear() {
            tracing::warn!(error = %e, "failed to clear the rejected token cache");
        }
        bearer = fetch_fresh_token(http).await.ok_or_else(no_credential)?;
        envelope = client.fetch_manifest(bearer.expose()).await;
    }

    // Why: minting writes the token to the cache before the gateway has seen
    // it. A fresh token the gateway then refuses (revoked PAT, deleted user)
    // must not be left behind as a "valid" cache entry, or the next run
    // replays a known-bad credential before rediscovering the same refusal.
    if is_unauthorized(&envelope)
        && let Err(e) = crate::auth::cache::clear()
    {
        tracing::warn!(error = %e, "failed to clear the refused token from the cache");
    }

    let credential = if was_cached {
        "both the cached credential and a freshly minted replacement"
    } else {
        "a freshly issued credential"
    };
    let envelope = envelope.map_err(|e| {
        map_gateway_error(
            e,
            "manifest",
            &RejectedCredential {
                credential,
                token: Some(&bearer),
            },
        )
    })?;

    Ok(ManifestFetch {
        client,
        bearer,
        envelope,
    })
}

const fn is_unauthorized<T>(result: &Result<T, GatewayError>) -> bool {
    matches!(
        result,
        Err(GatewayError::HttpStatus { status, .. }) if matches!(status.as_u16(), 401 | 403)
    )
}

pub(super) async fn verify_and_decode(
    fetch: &ManifestFetch,
    allow_unsigned: bool,
    allow_tofu: bool,
) -> Result<SignedManifest, SyncError> {
    if !allow_unsigned {
        let pubkey = resolve_pubkey(&fetch.client, allow_tofu).await?;
        verify_envelope(&fetch.envelope, pubkey.as_str()).map_err(map_manifest_error)?;
    }
    decode_payload(&fetch.envelope).map_err(map_manifest_error)
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
    let fetched = client.fetch_pubkey().await.map_err(|e| {
        map_gateway_error(
            e,
            "pubkey",
            &RejectedCredential {
                credential: "the request",
                token: None,
            },
        )
    })?;
    if let Err(e) = config::persist_pinned_pubkey(&fetched) {
        tracing::warn!(error = %e, "failed to persist pinned pubkey; next run will re-trust on first use");
    }
    let prefix: String = fetched.chars().take(12).collect();
    tracing::info!(
        "pinned manifest pubkey ({prefix}…) — future syncs will reject any pubkey rotation"
    );
    Ok(PinnedPubKey::new(fetched))
}

async fn fetch_fresh_token(http: &reqwest::Client) -> Option<Secret> {
    use crate::auth::providers::AuthError;
    use systemprompt_identifiers::SessionId;
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let session_id = SessionId::generate();
    let chain = crate::auth::provider_chain(&cfg);
    let mut not_configured: Vec<&'static str> = Vec::new();
    let mut had_failure = false;
    for p in &chain {
        match p.authenticate(&session_id, http).await {
            Ok(out) => {
                if let Err(e) = crate::auth::cache::write(&gateway, &out) {
                    tracing::warn!(error = %e, "failed to cache fresh token; will re-authenticate next call");
                }
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => {
                not_configured.push(p.name());
            },
            Err(e @ AuthError::Failed { .. }) => {
                had_failure = true;
                crate::stdio::diag(&format!("{}: {e}", p.name()));
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
        crate::stdio::diag(&format!(
            "no auth provider configured (tried: {tried}); run `{bin} login <sp-live-...>`"
        ));
    }
    None
}
