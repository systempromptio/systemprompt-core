//! On-disk sentinel marking that the user finished the setup wizard.
//!
//! Stored beside `first-run.json` in the bridge metadata directory rather
//! than in the config TOML, which `auth::setup::session_setup` rewrites
//! wholesale on every device link.
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
