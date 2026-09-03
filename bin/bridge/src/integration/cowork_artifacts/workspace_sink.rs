//! Stages every manifest artifact into the pre-trusted Cowork workspace as a
//! bundle: `manifest.json` plus one `<id>.html` per record.
//!
//! Cowork's `create_artifact` takes an `html_path` inside the session workspace
//! or a connected folder, and the workspace named by `allowedWorkspaceFolders`
//! is connected by default. Staging there lets the setup skills install
//! dashboards with file tools alone — no shell, which Cowork may not grant.
//!
//! [`stage_bundle`] resolves that destination itself and is called directly by
//! `CoworkArtifactsSync::apply`, before the session directory is resolved: it
//! needs nothing from that directory, and being driven through
//! [`super::emit::write_artifacts`] made it unreachable until Cowork had
//! created one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use systemprompt_models::bridge::cowork_artifact::{
    CoworkArtifactBundleManifest, CoworkArtifactBundleRecord,
};

use crate::config::paths;
use crate::gateway::manifest::ArtifactEntry;
use crate::hash::safe_id_segment;
use crate::host_sync::ApplyError;

pub const BUNDLE_MANIFEST_FILE: &str = "manifest.json";

#[must_use]
pub fn bundle_is_current(dir: &Path, artifacts: &[ArtifactEntry]) -> bool {
    let expected: BTreeSet<&str> = artifacts
        .iter()
        .map(|a| a.id.as_str())
        .filter(|id| safe_id_segment(id))
        .collect();
    let staged = staged_ids(dir);
    if staged.len() != expected.len() || !staged.iter().all(|id| expected.contains(id.as_str())) {
        return false;
    }
    let Ok(bytes) = std::fs::read(dir.join(BUNDLE_MANIFEST_FILE)) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_slice::<CoworkArtifactBundleManifest>(&bytes) else {
        return false;
    };
    let versions: BTreeSet<(String, String)> = manifest
        .artifacts
        .iter()
        .map(|r| (r.id.as_str().to_owned(), r.version.clone()))
        .collect();
    artifacts
        .iter()
        .filter(|a| safe_id_segment(a.id.as_str()))
        .all(|a| versions.contains(&(a.id.as_str().to_owned(), a.version.clone())))
}

pub fn write_bundle(dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
    std::fs::create_dir_all(dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let expected: BTreeSet<&str> = artifacts.iter().map(|a| a.id.as_str()).collect();
    for stale in staged_ids(dir) {
        if expected.contains(stale.as_str()) {
            continue;
        }
        let path = dir.join(format!("{stale}.html"));
        std::fs::remove_file(&path).map_err(|e| ApplyError::Io {
            context: format!("remove stale staged page {}", path.display()),
            source: e,
        })?;
    }
    let mut records = Vec::with_capacity(artifacts.len());
    for artifact in artifacts {
        let id = artifact.id.as_str();
        if !safe_id_segment(id) {
            tracing::warn!(artifact_id = %id, "cowork artifacts: unsafe id; not staged in the workspace");
            continue;
        }
        let page = dir.join(format!("{id}.html"));
        crate::fsutil::atomic_write_0644(&page, artifact.content.as_bytes()).map_err(|e| {
            ApplyError::Io {
                context: format!("write {}", page.display()),
                source: e,
            }
        })?;
        records.push(CoworkArtifactBundleRecord::from(artifact));
    }
    let manifest = CoworkArtifactBundleManifest { artifacts: records };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| ApplyError::Serialize {
        what: BUNDLE_MANIFEST_FILE.into(),
        source: e,
    })?;
    let path = dir.join(BUNDLE_MANIFEST_FILE);
    crate::fsutil::atomic_write_0644(&path, &bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

// Why: driving this through `emit::write_artifacts` made it unreachable until
// Cowork had created a session dir, so a fresh install staged no dashboards and
// the setup skill read the empty folder as "the bridge has never synced".
pub fn stage_bundle(artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
    let Some(dir) = paths::workspace_artifacts_dir() else {
        tracing::info!("cowork artifacts: no workspace dir on this host; bundle not staged");
        return Ok(());
    };
    // Why: same contract as `emit::write_artifacts` — an enabled host sending an
    // empty set signals an upstream scoping bug, so the staged bundle is
    // preserved rather than cleared.
    if artifacts.is_empty() {
        tracing::warn!(
            workspace_dir = %dir.display(),
            "cowork artifacts: enabled host sent an empty artifact set — preserving staged workspace bundle (not clearing); check gateway marketplace scoping/gating"
        );
        return Ok(());
    }
    if dir.join(BUNDLE_MANIFEST_FILE).is_file() && bundle_is_current(&dir, artifacts) {
        tracing::info!(
            count = artifacts.len(),
            workspace_dir = %dir.display(),
            "cowork artifacts: workspace bundle up to date, skipping"
        );
        return Ok(());
    }
    write_bundle(&dir, artifacts)?;
    tracing::info!(
        count = artifacts.len(),
        workspace_dir = %dir.display(),
        "cowork artifacts: workspace bundle staged"
    );
    Ok(())
}

pub fn remove_bundle(dir: &Path) -> Result<(), ApplyError> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| ApplyError::Io {
            context: format!("remove {}", dir.display()),
            source: e,
        })?;
    }
    Ok(())
}

fn staged_ids(dir: &Path) -> BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("html") {
                return None;
            }
            path.file_stem().and_then(|s| s.to_str()).map(str::to_owned)
        })
        .collect()
}
