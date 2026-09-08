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
    cfg: &config::Config,
    err: GatewayError,
    endpoint: &'static str,
    rejected: &RejectedCredential<'_>,
) -> SyncError {
    match err {
        GatewayError::HttpStatus { status, .. } if matches!(status.as_u16(), 401 | 403) => {
            unauthorized(cfg, endpoint, status.as_u16(), rejected)
        },
        GatewayError::ManifestDecode(e) if e.is_decode() => SyncError::ManifestShape(e.to_string()),
        e @ GatewayError::ManifestEnvelopeShape { .. } => SyncError::ManifestShape(e.to_string()),
        other => SyncError::Network(other.to_string()),
    }
}

fn unauthorized(
    cfg: &config::Config,
    endpoint: &'static str,
    status: u16,
    rejected: &RejectedCredential<'_>,
) -> SyncError {
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
        gateway: config::gateway_url_or_default(cfg).to_string(),
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
        other => SyncError::ManifestShape(other.to_string()),
    }
}

fn signature_failure(
    err: ManifestError,
    client: &GatewayClient,
    source: config::PinSource,
) -> SyncError {
    match err {
        ManifestError::SchemaTooNew { .. }
        | ManifestError::BridgeTooOld { .. }
        | ManifestError::PayloadParse(_) => map_manifest_error(err),
        other => SyncError::SignatureFailed {
            detail: other.to_string(),
            gateway: client.base_url().to_string(),
            pin_source: source.label(),
            fix: match source {
                config::PinSource::Policy => {
                    "Update the policy-supplied key (env var or managed policy) to the key this \
                     gateway serves at /v1/bridge/pubkey."
                },
                config::PinSource::Operator => {
                    "Remove the [sync.trust] section from the config file, or run \
                     `install --apply --pubkey <base64>`, then sync again."
                },
            },
        },
    }
}

pub(super) struct ManifestFetch {
    pub client: GatewayClient,
    pub bearer: Secret,
    pub envelope: SignedManifestEnvelope,
    pub config: config::Config,
}

pub(super) async fn fetch_authenticated_manifest(
    http: &reqwest::Client,
) -> Result<ManifestFetch, SyncError> {
    let cfg = config::load()?;
    let gateway = config::gateway_url_or_default(&cfg);
    let client = GatewayClient::new(gateway.clone(), http.clone());

    let cached = crate::auth::cache::read_for(&cfg, &gateway, 30)
        .map_err(SyncError::CredentialCache)?
        .map(|out| out.token);
    let was_cached = cached.is_some();
    let mut bearer = match cached {
        Some(token) => token,
        None => fetch_fresh_token(http, &cfg).await?,
    };

    let mut envelope = client.fetch_manifest(bearer.expose()).await;

    if is_unauthorized(&envelope) && was_cached {
        tracing::warn!("gateway refused the cached token; discarding it and re-authenticating");
        crate::auth::cache::clear().map_err(SyncError::CredentialCache)?;
        bearer = fetch_fresh_token(http, &cfg).await?;
        envelope = client.fetch_manifest(bearer.expose()).await;
    }

    if is_unauthorized(&envelope) {
        crate::auth::cache::clear().map_err(SyncError::CredentialCache)?;
    }

    let credential = if was_cached {
        "both the cached credential and a freshly minted replacement"
    } else {
        "a freshly issued credential"
    };
    let envelope = envelope.map_err(|e| {
        map_gateway_error(
            &cfg,
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
        config: cfg,
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
    if allow_unsigned {
        return decode_payload(&fetch.envelope).map_err(map_manifest_error);
    }
    let state = config::trust::pinned_pubkey_state_for(&fetch.config, fetch.client.base_url())?;
    let (pubkey, source, newly_trusted) = match state {
        config::PinnedPubkeyState::Pinned { key, source } => (key, source, false),
        config::PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        } => {
            return Err(SyncError::PubkeyStale {
                pinned_for,
                current,
            });
        },
        config::PinnedPubkeyState::Unpinned if allow_tofu => {
            let key = fetch.client.fetch_pubkey().await.map_err(|e| {
                map_gateway_error(
                    &fetch.config,
                    e,
                    "pubkey",
                    &RejectedCredential {
                        credential: "the request",
                        token: None,
                    },
                )
            })?;
            (PinnedPubKey::new(key), config::PinSource::Operator, true)
        },
        config::PinnedPubkeyState::Unpinned => return Err(SyncError::PubkeyNotPinned),
    };
    verify_envelope(&fetch.envelope, pubkey.as_str())
        .map_err(|e| signature_failure(e, &fetch.client, source))?;
    let manifest = decode_payload(&fetch.envelope).map_err(map_manifest_error)?;
    if newly_trusted {
        config::persist_pinned_pubkey(fetch.client.base_url(), pubkey.as_str())?;
    }
    Ok(manifest)
}

async fn fetch_fresh_token(
    http: &reqwest::Client,
    cfg: &config::Config,
) -> Result<Secret, SyncError> {
    let out = crate::auth::mint_fresh(cfg, &systemprompt_identifiers::SessionId::generate(), http)
        .await
        .map_err(|e| match e {
            crate::auth::ChainError::NoneSucceeded => SyncError::NoCredential {
                bin: crate::brand::brand().binary_name,
            },
            other => SyncError::Authentication(other),
        })?;
    Ok(out.token)
}
