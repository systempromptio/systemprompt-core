//! On-disk sentinel marking that the user finished the setup wizard.
//!
//! `agents_onboarded` lived only in the in-memory snapshot, so pressing Finish
//! bought nothing beyond the current process: on the next launch the only thing
//! keeping the wizard away was "some host still reports a profile installed".
//! Uninstall the last profile, or probe it as stale, and a user who had already
//! completed setup was put back through it.
//!
//! Stored beside `first-run.json` in the bridge metadata directory for the same
//! reason that record is: `auth::setup::session_setup` rewrites the config TOML
//! wholesale on every device link, so a flag kept there would be erased by the
//! very event that most often precedes setting it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardedRecord {
    pub completed_at: String,
    pub app_version: String,
}

fn sentinel_path() -> Option<PathBuf> {
    paths::bridge_metadata_dir().map(|d| d.join(paths::ONBOARDED_SENTINEL))
}

/// Has the user completed the setup wizard on this install?
#[must_use]
pub fn is_complete() -> bool {
    let Some(path) = sentinel_path() else {
        return false;
    };
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    serde_json::from_slice::<OnboardedRecord>(&bytes)
        .inspect_err(
            |e| tracing::warn!(error = %e, "onboarding sentinel is corrupt; treating as absent"),
        )
        .is_ok()
}

/// Records that setup finished. Best-effort: a metadata directory we cannot
/// write is a reason to re-show the wizard next launch, not to fail the click.
pub fn mark_complete() {
    let Some(path) = sentinel_path() else {
        tracing::warn!("no metadata dir; onboarding sentinel not written");
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::warn!(error = %e, dir = %parent.display(), "onboarding: sentinel parent mkdir failed");
        return;
    }
    let record = OnboardedRecord {
        completed_at: chrono::Utc::now().to_rfc3339(),
        app_version: crate::brand::brand().version.to_owned(),
    };
    match serde_json::to_vec_pretty(&record) {
        Ok(bytes) => {
            if let Err(e) = fs::write(&path, bytes) {
                tracing::warn!(error = %e, path = %path.display(), "onboarding: sentinel write failed");
            }
        },
        Err(e) => tracing::warn!(error = %e, "onboarding: sentinel serialize failed"),
    }
}
