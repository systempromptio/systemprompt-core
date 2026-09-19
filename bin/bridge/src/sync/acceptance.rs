//! Shared publication guards for full sync and registry refresh.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ValidatedUrl;

use super::{
    LastSyncState, SyncError, check_replay, check_skew, ensure_not_superseded, prior_checkpoint,
};
use crate::config::paths;
use crate::gateway::manifest::{SignedManifest, decode_payload};

pub(super) fn accept(
    manifest: &SignedManifest,
    gateway: &ValidatedUrl,
    force_replay: bool,
) -> Result<LastSyncState, SyncError> {
    let meta = paths::bridge_metadata_dir().ok_or(SyncError::PathUnresolvable)?;
    let prior = prior_checkpoint(&meta.join(paths::LAST_SYNC_SENTINEL), gateway)?;
    if !force_replay {
        check_skew(manifest.not_before, chrono::Utc::now())?;
        if prior.manifest_version.as_ref() != Some(&manifest.manifest_version) {
            check_replay(&prior, &manifest.manifest_version)?;
        }
        if let Some(fragment) = crate::mcp_registry::read_envelope().map_err(|source| {
            SyncError::ApplyFailed(Box::new(super::apply::ApplyError::Io {
                context: "read accepted manifest envelope".to_owned(),
                source,
            }))
        })? && crate::mcp_registry::same_origin(&fragment.gateway, gateway)
        {
            let accepted =
                decode_payload(&fragment.envelope).map_err(super::manifest::map_manifest_error)?;
            if manifest.manifest_version < accepted.manifest_version {
                return Err(SyncError::ReplayedManifest {
                    last: accepted.manifest_version.to_string(),
                    incoming: manifest.manifest_version.to_string(),
                });
            }
        }
    }
    ensure_not_superseded(gateway)?;
    Ok(prior)
}
