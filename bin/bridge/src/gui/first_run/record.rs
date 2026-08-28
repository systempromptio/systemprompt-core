//! On-disk sentinel marking that first-use provisioning has already run.
//!
//! Lives in the bridge metadata directory alongside `last-sync.json` rather
//! than in the config TOML: `auth::setup::session_setup` rewrites that file
//! wholesale on every device link, so a flag stored there would be erased by
//! the very event that sets it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::paths;

use super::state::{FirstRunState, StepStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostOutcome {
    pub host_id: String,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstRunRecord {
    pub completed_at: String,
    pub app_version: String,
    #[serde(default)]
    pub hosts: Vec<HostOutcome>,
    #[serde(default)]
    pub sync_ok: bool,
}

fn sentinel_path() -> Option<PathBuf> {
    paths::bridge_metadata_dir().map(|d| d.join(paths::FIRST_RUN_SENTINEL))
}

#[must_use]
pub fn read() -> Option<FirstRunRecord> {
    let bytes = fs::read(sentinel_path()?).ok()?;
    serde_json::from_slice(&bytes)
        .inspect_err(
            |e| tracing::warn!(error = %e, "first-run sentinel is corrupt; treating as absent"),
        )
        .ok()
}

pub fn write(state: &FirstRunState) {
    let Some(path) = sentinel_path() else {
        tracing::warn!("no metadata dir; first-run sentinel not written");
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::warn!(error = %e, dir = %parent.display(), "first-run: sentinel parent mkdir failed");
        return;
    }
    let record = FirstRunRecord {
        completed_at: chrono::Utc::now().to_rfc3339(),
        app_version: crate::brand::brand().version.to_owned(),
        hosts: state
            .hosts
            .iter()
            .map(|h| HostOutcome {
                host_id: h.host_id.clone(),
                status: h.status.as_str().to_owned(),
                error: h.error.clone(),
            })
            .collect(),
        sync_ok: state.sync == StepStatus::Done,
    };
    match serde_json::to_vec_pretty(&record) {
        Ok(bytes) => {
            if let Err(e) = fs::write(&path, bytes) {
                tracing::warn!(error = %e, path = %path.display(), "first-run: sentinel write failed");
            }
        },
        Err(e) => tracing::warn!(error = %e, "first-run: sentinel serialize failed"),
    }
}

fn tray_notice_path() -> Option<PathBuf> {
    paths::bridge_metadata_dir().map(|d| d.join(paths::TRAY_NOTICE_SENTINEL))
}

#[must_use]
pub fn tray_notice_shown() -> bool {
    tray_notice_path().is_some_and(|p| p.exists())
}

pub fn mark_tray_notice_shown() {
    let Some(path) = tray_notice_path() else {
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        tracing::debug!(error = %e, "tray-notice sentinel parent mkdir failed");
        return;
    }
    if let Err(e) = fs::write(&path, b"{}") {
        tracing::debug!(error = %e, "tray-notice sentinel write failed");
    }
}
