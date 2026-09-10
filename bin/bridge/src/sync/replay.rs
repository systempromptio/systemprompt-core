//! Replay-protection state for manifest version monotonicity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::SyncError;
use crate::gateway::manifest_version::ManifestVersion;
use crate::last_sync::LastSyncState;

pub const SKEW_WINDOW_MINUTES: i64 = 5;

pub fn check_replay(last: &LastSyncState, incoming: &ManifestVersion) -> Result<(), SyncError> {
    if let Some(prev) = last.last_applied_manifest_version.as_ref()
        && incoming <= prev
    {
        return Err(SyncError::ReplayedManifest {
            last: prev.to_string(),
            incoming: incoming.to_string(),
        });
    }
    Ok(())
}

pub fn check_skew(not_before: &str, now: chrono::DateTime<chrono::Utc>) -> Result<(), SyncError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(not_before).map_err(|_parse| {
        SyncError::ManifestSkew {
            not_before: not_before.to_owned(),
            now: now.to_rfc3339(),
        }
    })?;
    let nb_utc = parsed.with_timezone(&chrono::Utc);
    let window = chrono::Duration::minutes(SKEW_WINDOW_MINUTES);
    let delta = nb_utc.signed_duration_since(now);
    if delta > window || delta < -window {
        return Err(SyncError::ManifestSkew {
            not_before: not_before.to_owned(),
            now: now.to_rfc3339(),
        });
    }
    Ok(())
}
