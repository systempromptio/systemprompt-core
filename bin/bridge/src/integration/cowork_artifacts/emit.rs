//! IO layer for the Cowork artifacts emitter: resolves the target dir, hashes
//! the artifact set for idempotency, and drives the active [`ArtifactSink`].
//!
//! Idempotency mirrors the synthetic-plugin writer: `version.json` carries a
//! content hash of the artifact set and is written *last*, so it doubles as the
//! completion marker the next sync's skip check keys on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::paths;
use crate::gateway::manifest::ArtifactEntry;
use crate::integration::cowork_plugins::resolve_target;
use crate::sync::apply::ApplyError;
use crate::sync::hash::sha256_hex;

use super::sink::{ArtifactSink, FileSink, SeedStaging};

const VERSION_FILE: &str = "version.json";

#[must_use]
pub fn active_sinks() -> &'static [&'static dyn ArtifactSink] {
    &[&FileSink, &SeedStaging]
}

/// `None` means no Cowork install detected; callers treat it as a no-op.
#[must_use]
pub fn resolve_artifacts_dir() -> Option<PathBuf> {
    let target = resolve_target()?;
    Some(target.session_org_dir.join(paths::COWORK_ARTIFACTS_SUBDIR))
}

/// Hashes only identity fields, independent of the sink's on-disk rendering,
/// so a sink swap does not force a rewrite.
#[must_use]
pub fn artifacts_version(artifacts: &[ArtifactEntry]) -> String {
    let mut sorted: Vec<&ArtifactEntry> = artifacts.iter().collect();
    sorted.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let mut buf = String::new();
    for a in sorted {
        buf.push_str(a.id.as_str());
        buf.push('\u{0}');
        buf.push_str(&a.version);
        buf.push('\u{0}');
        buf.push_str(a.sha256.as_str());
        buf.push('\u{0}');
    }
    buf.push('\u{1}');
    sha256_hex(buf.as_bytes())
}

fn read_existing_version(dir: &Path) -> Option<String> {
    let bytes = fs::read(dir.join(VERSION_FILE)).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("version")?.as_str().map(str::to_owned)
}

fn write_version_json(dir: &Path, version: &str) -> Result<(), ApplyError> {
    let bytes =
        serde_json::to_vec_pretty(&serde_json::json!({ "version": version })).map_err(|e| {
            ApplyError::Serialize {
                what: VERSION_FILE.into(),
                source: e,
            }
        })?;
    let path = dir.join(VERSION_FILE);
    crate::fsutil::atomic_write_0600(&path, &bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

pub fn write_artifacts(
    dir: &Path,
    sinks: &[&dyn ArtifactSink],
    artifacts: &[ArtifactEntry],
) -> Result<(), ApplyError> {
    if artifacts.is_empty() {
        tracing::info!(target_dir = %dir.display(), "cowork artifacts: empty set, clearing store");
        return remove_dir(dir);
    }

    let version = artifacts_version(artifacts);
    if read_existing_version(dir).as_deref() == Some(version.as_str())
        && sinks.iter().all(|s| s.is_materialized(dir))
    {
        tracing::info!(
            count = artifacts.len(),
            target_dir = %dir.display(),
            artifacts_version = %version,
            "cowork artifacts: up to date, skipping"
        );
        return Ok(());
    }

    fs::create_dir_all(dir).map_err(|e| ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;

    for sink in sinks {
        sink.write(dir, artifacts)?;
    }
    write_version_json(dir, &version)?;
    tracing::info!(
        count = artifacts.len(),
        target_dir = %dir.display(),
        artifacts_version = %version,
        "cowork artifacts: store written"
    );
    Ok(())
}

pub fn remove_dir(dir: &Path) -> Result<(), ApplyError> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| ApplyError::Io {
            context: format!("remove {}", dir.display()),
            source: e,
        })?;
    }
    Ok(())
}
