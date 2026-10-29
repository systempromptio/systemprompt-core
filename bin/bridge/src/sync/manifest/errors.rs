//! Mapping gateway and manifest failures onto sync errors, including the
//! operator-facing detail for a rejected credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::error::{ManifestShapeCause, SyncError};
use crate::auth::secret::Secret;
use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::ManifestError;

pub(super) struct RejectedCredential<'a> {
    pub(super) credential: &'static str,
    pub(super) token: Option<&'a Secret>,
}

pub(super) fn map_gateway_error(
    cfg: &config::Config,
    err: GatewayError,
    endpoint: &'static str,
    rejected: &RejectedCredential<'_>,
) -> SyncError {
    match err {
        GatewayError::Rejected { status, .. } if matches!(status.as_u16(), 401 | 403) => {
            unauthorized(cfg, endpoint, status.as_u16(), rejected)
        },
        GatewayError::ManifestDecode(e) if e.is_decode() => SyncError::ManifestShape(
            ManifestShapeCause::Gateway(Box::new(GatewayError::ManifestDecode(e))),
        ),
        e @ GatewayError::ManifestEnvelopeShape { .. } => {
            SyncError::ManifestShape(ManifestShapeCause::Gateway(Box::new(e)))
        },
        other => SyncError::Gateway(other),
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
        Ok(p) => (p.config_file.display().to_string(), p.pat_location()),
        Err(_) => ("<unresolvable>".to_owned(), "<unresolvable>".to_owned()),
    };
    SyncError::GatewayUnauthorized(Box::new(super::super::error::CredentialRejection {
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

pub(in crate::sync) fn map_manifest_error(err: ManifestError) -> SyncError {
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
        ManifestError::BridgeBelowWindow { local, min } => {
            SyncError::BridgeBelowWindow { local, min }
        },
        ManifestError::BridgeAboveWindow { local, max } => {
            SyncError::BridgeAboveWindow { local, max }
        },
        other => SyncError::ManifestShape(ManifestShapeCause::Manifest(other)),
    }
}

pub(super) fn signature_failure(
    err: ManifestError,
    client: &GatewayClient,
    source: config::PinSource,
) -> SyncError {
    match err {
        ManifestError::SchemaTooNew { .. }
        | ManifestError::BridgeTooOld { .. }
        | ManifestError::BridgeBelowWindow { .. }
        | ManifestError::BridgeAboveWindow { .. }
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
