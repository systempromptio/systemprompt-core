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

/// Whether a version was accepted before the one being judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorAcceptance {
    None,
    Accepted,
}

impl PriorAcceptance {
    #[must_use]
    pub const fn of(accepted: bool) -> Self {
        if accepted { Self::Accepted } else { Self::None }
    }
}

// Why: once a version has been accepted, an envelope's age is governed by the
// monotonic manifest version, so only a date too far ahead of this clock is
// refused (a gateway may serve a cached envelope hours old). With nothing
// accepted yet there is no version to bound age, so the envelope must also be
// fresh. The policy writer applies the same rule to its own record.
#[must_use]
pub fn dated_beyond_skew(
    not_before: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
    prior: PriorAcceptance,
) -> bool {
    let window = chrono::Duration::minutes(SKEW_WINDOW_MINUTES);
    let ahead = not_before.signed_duration_since(now);
    match prior {
        PriorAcceptance::Accepted => ahead > window,
        PriorAcceptance::None => ahead > window || ahead < -window,
    }
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
