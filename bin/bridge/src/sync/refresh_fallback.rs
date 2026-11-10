//! Whether a failed manifest refresh lets a profile write or repair go on
//! with the envelope last verified, or stops it.
//!
//! Only a manifest that arrived and failed verification or acceptance stops
//! the work: going on would mean writing past a refusal. Every other failure
//! (no gateway, no credential yet, a disabled bridge, a local fault) keeps
//! the envelope last verified, which the policy writer verifies again. The
//! match below names every [`SyncError`] variant, so a new one is classified
//! deliberately rather than by a default.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::gateway::GatewayError;

/// Why a refresh kept the stored envelope instead of a fresh one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeptStored {
    Unreachable,
    NoCredential,
    Disabled,
    Unavailable,
}

/// How a failed refresh is acted on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshFailure {
    Kept(KeptStored),
    Rejected,
}

fn transport(error: &reqwest::Error) -> bool {
    error.status().is_none() && (error.is_connect() || error.is_timeout() || error.is_request())
}

fn of_gateway(error: &GatewayError) -> RefreshFailure {
    match error {
        GatewayError::PubkeyDecode(_)
        | GatewayError::PubkeyMissing
        | GatewayError::ManifestDecode(_)
        | GatewayError::ManifestEnvelopeShape { .. }
        | GatewayError::ManifestNotModifiedWithoutCache => RefreshFailure::Rejected,
        GatewayError::ManifestFetch(e) | GatewayError::PubkeyFetch(e) if transport(e) => {
            RefreshFailure::Kept(KeptStored::Unreachable)
        },
        _ => RefreshFailure::Kept(KeptStored::Unavailable),
    }
}

#[must_use]
pub fn classify(error: &SyncError) -> RefreshFailure {
    use RefreshFailure::{Kept, Rejected};
    match error {
        SyncError::UnsignedRefusedPinned { .. }
        | SyncError::SignatureFailed { .. }
        | SyncError::SchemaTooNew { .. }
        | SyncError::ManifestShape(_)
        | SyncError::BridgeTooOld { .. }
        | SyncError::BridgeBelowWindow { .. }
        | SyncError::BridgeAboveWindow { .. }
        | SyncError::PubkeyStale { .. }
        | SyncError::PubkeyNotPinned
        | SyncError::ReplayedManifest { .. }
        | SyncError::ManifestSkew { .. }
        | SyncError::ReplayStateCorrupt(_) => Rejected,
        SyncError::Gateway(gateway) => of_gateway(gateway),
        SyncError::Disabled { .. } => Kept(KeptStored::Disabled),
        SyncError::NoCredential { .. } | SyncError::GatewayUnauthorized(_) => {
            Kept(KeptStored::NoCredential)
        },
        SyncError::Authentication(chain) if chain.is_terminal() => Kept(KeptStored::NoCredential),
        SyncError::Authentication(_) => Kept(KeptStored::Unreachable),
        SyncError::CredentialCache(_)
        | SyncError::Partial(_)
        | SyncError::Persistence { .. }
        | SyncError::Config(_)
        | SyncError::Trust(_)
        | SyncError::Provision(_)
        | SyncError::OrgPluginsDenied { .. }
        | SyncError::OrgPluginsCreate { .. }
        | SyncError::Profile(_)
        | SyncError::ClaudeCodeSettings { .. }
        | SyncError::Cancelled { .. }
        | SyncError::GatewayProfile(_)
        | SyncError::OrgPluginsNeedElevation { .. }
        | SyncError::PathUnresolvable
        | SyncError::PathMissing { .. }
        | SyncError::ApplyFailed(_)
        | SyncError::Superseded { .. } => Kept(KeptStored::Unavailable),
    }
}
