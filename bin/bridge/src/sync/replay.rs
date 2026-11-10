//! Replay-protection state for manifest version monotonicity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::gateway::manifest_version::ManifestVersion;
use crate::last_sync::LastSyncState;

pub const SKEW_WINDOW_MINUTES: i64 = 5;

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

// Why: a gateway may serve a cached envelope whose `not_before` is hours
// old; its age is governed by the monotonic manifest version, so only a date
// too far ahead of this clock is refused. The policy writer applies the same
// rule.
#[must_use]
pub fn dated_beyond_skew(
    not_before: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    not_before.signed_duration_since(now) > chrono::Duration::minutes(SKEW_WINDOW_MINUTES)
}

pub fn check_skew(
    not_before: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), SyncError> {
    if dated_beyond_skew(not_before, now) {
        return Err(SyncError::ManifestSkew {
            not_before: not_before.to_rfc3339(),
            now: now.to_rfc3339(),
        });
    }
    Ok(())
}
