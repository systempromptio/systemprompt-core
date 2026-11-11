//! Replay-protection state for manifest version monotonicity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::gateway::envelope_skew::{PriorAcceptance, dated_beyond_skew};
use crate::gateway::manifest_version::ManifestVersion;
use crate::last_sync::LastSyncState;

pub fn check_replay(last: &LastSyncState, incoming: &ManifestVersion) -> Result<(), SyncError> {
    if let Some(prev) = last.manifest_version.as_ref()
        && incoming <= prev
    {
        return Err(SyncError::ReplayedManifest {
            last: prev.to_string(),
            incoming: incoming.to_string(),
        });
    }
    Ok(())
}

pub fn check_skew(
    not_before: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    prior: PriorAcceptance,
) -> Result<(), SyncError> {
    if dated_beyond_skew(not_before, now, prior) {
        return Err(SyncError::ManifestSkew {
            not_before: not_before.to_rfc3339(),
            now: now.to_rfc3339(),
        });
    }
    Ok(())
}
