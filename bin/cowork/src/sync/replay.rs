use super::error::SyncError;
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub const SKEW_WINDOW_MINUTES: i64 = 5;

#[derive(Default, Debug, Clone, Deserialize)]
pub struct LastSyncState {
    #[serde(default)]
    pub last_applied_manifest_version: Option<String>,
}

#[must_use]
pub fn read_last_sync(path: &Path) -> LastSyncState {
    let Ok(bytes) = fs::read(path) else {
        return LastSyncState::default();
    };
    serde_json::from_slice::<LastSyncState>(&bytes).unwrap_or_default()
}

pub fn check_replay(last: &LastSyncState, incoming: &str) -> Result<(), SyncError> {
    if let Some(prev) = last.last_applied_manifest_version.as_deref() {
        if incoming <= prev {
            return Err(SyncError::ReplayedManifest {
                last: prev.to_string(),
                incoming: incoming.to_string(),
            });
        }
    }
    Ok(())
}

pub fn check_skew(not_before: &str, now: chrono::DateTime<chrono::Utc>) -> Result<(), SyncError> {
    let parsed =
        chrono::DateTime::parse_from_rfc3339(not_before).map_err(|_| SyncError::ManifestSkew {
            not_before: not_before.to_string(),
            now: now.to_rfc3339(),
        })?;
    let nb_utc = parsed.with_timezone(&chrono::Utc);
    let window = chrono::Duration::minutes(SKEW_WINDOW_MINUTES);
    let delta = nb_utc.signed_duration_since(now);
    if delta > window || delta < -window {
        return Err(SyncError::ManifestSkew {
            not_before: not_before.to_string(),
            now: now.to_rfc3339(),
        });
    }
    Ok(())
}
