//! Signed-manifest fetch and public-key resolution for plugin sync.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod errors;

use super::error::SyncError;
use crate::auth::secret::Secret;
use crate::config;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::{
    SignedManifest, SignedManifestEnvelope, decode_payload, verify_envelope,
};
use crate::gateway::{Freshness, GatewayClient};
use crate::ids::PinnedPubKey;
pub(super) use errors::map_manifest_error;
use errors::{RejectedCredential, map_gateway_error, signature_failure};

pub(super) struct ManifestFetch {
    pub client: GatewayClient,
    pub bearer: Secret,
    pub envelope: SignedManifestEnvelope,
    pub config: config::Config,
    pub etag: Option<String>,
    // Why: The gateway answered 304 in this run for the stored envelope's ETag, so
    // `envelope` is the stored one, revalidated rather than freshly issued.
    pub revalidated: bool,
}

pub(super) async fn fetch_authenticated_manifest(
    http: &reqwest::Client,
    freshness: Freshness,
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

    let stored = super::manifest_cache::stored_envelope(&gateway);
    let if_none_match = stored.as_ref().and_then(|f| f.etag.clone());
    let mut envelope = client
        .fetch_manifest_conditional(&bearer, freshness, if_none_match.as_deref())
        .await;

    if is_unauthorized(&envelope) && was_cached {
        tracing::warn!("gateway refused the cached token; discarding it and re-authenticating");
        crate::auth::cache::clear().map_err(SyncError::CredentialCache)?;
        bearer = fetch_fresh_token(http, &cfg).await?;
        envelope = client
            .fetch_manifest_conditional(&bearer, freshness, if_none_match.as_deref())
            .await;
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

    let (envelope, etag, revalidated) = super::manifest_cache::resolve(envelope, stored)?;

    Ok(ManifestFetch {
        client,
        bearer,
        envelope,
        config: cfg,
        etag,
        revalidated,
    })
}

const fn is_unauthorized<T>(result: &Result<T, GatewayError>) -> bool {
    matches!(
        result,
        Err(GatewayError::Rejected { status, .. }) if matches!(status.as_u16(), 401 | 403)
    )
}

pub(super) async fn verify_and_decode(
    fetch: &ManifestFetch,
    allow_unsigned: bool,
    allow_tofu: bool,
) -> Result<SignedManifest, SyncError> {
    let state = config::trust::pinned_pubkey_state_for(&fetch.config, fetch.client.base_url())?;
    if allow_unsigned {
        if let config::PinnedPubkeyState::Pinned { source, .. } = &state {
            return Err(SyncError::UnsignedRefusedPinned {
                gateway: fetch.client.base_url().as_str().to_owned(),
                pin_source: source.label(),
            });
        }
        let manifest = decode_payload(&fetch.envelope).map_err(map_manifest_error)?;
        admit_release(&manifest)?;
        return Ok(manifest);
    }
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
    record_delivered_instance(fetch.client.base_url(), &manifest)?;
    admit_release(&manifest)?;
    Ok(manifest)
}

fn record_delivered_instance(
    gateway: &systemprompt_identifiers::ValidatedUrl,
    manifest: &SignedManifest,
) -> Result<(), SyncError> {
    let profile = config::gateways::active().map_err(|e| SyncError::GatewayProfile(Box::new(e)))?;
    config::gateways::write_delivered(
        &profile,
        &config::gateways::DeliveredInstance {
            gateway: Some(gateway.clone()),
            window: manifest.bridge_versions.clone(),
            branding: manifest.branding.clone(),
            allow_user_disable: manifest.allow_user_disable,
        },
    )
    .map_err(|e| SyncError::GatewayProfile(Box::new(e)))
}

fn admit_release(manifest: &SignedManifest) -> Result<(), SyncError> {
    let local = crate::brand::brand().version;
    match crate::gateway::manifest::check_release_window(manifest, local)
        .map_err(map_manifest_error)?
    {
        crate::gateway::manifest::WindowVerdict::Admitted => Ok(()),
        crate::gateway::manifest::WindowVerdict::AboveTolerated { max } => {
            tracing::warn!(
                %local,
                %max,
                "this bridge release is newer than the instance admits; install the offered \
                 release to return inside its window"
            );
            Ok(())
        },
    }
}

pub(super) fn ensure_enabled(config: &config::Config) -> Result<(), SyncError> {
    if config.is_disabled() {
        return Err(SyncError::Disabled {
            bin: crate::brand::brand().binary_name,
        });
    }
    Ok(())
}

async fn fetch_fresh_token(
    http: &reqwest::Client,
    cfg: &config::Config,
) -> Result<Secret, SyncError> {
    let out = crate::auth::mint_fresh(
        cfg,
        &crate::auth::session_identity::stable_session_id(cfg),
        http,
    )
    .await
    .map_err(|e| match e {
        crate::auth::ChainError::NoneSucceeded => SyncError::NoCredential {
            bin: crate::brand::brand().binary_name,
        },
        other => SyncError::Authentication(other),
    })?;
    Ok(out.token)
}
