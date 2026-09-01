//! Pluggable write targets for Cowork library artifacts.
//!
//! The live Cowork library ingests artifacts only via its native
//! `create_artifact` tool, so [`super::emit::active_sinks`] writes through
//! both implementations of [`ArtifactSink`]:
//!
//! - [`FileSink`] writes an on-disk store keyed by artifact id (read by the
//!   bridge GUI's Artifacts listing; usable directly if the library ever
//!   becomes file-writable). The store is a projection of the manifest, not an
//!   accumulation: each write replaces it, so an id the manifest has stopped
//!   carrying is dropped.
//! - [`SeedStaging`] drops one record per artifact into a staging dir for the
//!   first-run `create_artifact` seed skill to consume.
//!
//! Store paths live only here — writers use `CoworkLibraryArtifactRecord`,
//! readers (the GUI listing) use [`StoredArtifactSummary`] via
//! [`read_library_store`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;
use systemprompt_models::bridge::cowork_artifact::CoworkLibraryArtifactRecord;

use crate::gateway::manifest::ArtifactEntry;
use crate::hash::safe_id_segment;
use crate::host_sync::ApplyError;

pub const LIBRARY_STORE_FILE: &str = "library.json";

pub const STAGING_SUBDIR: &str = "staging";

/// Read-side view of a store entry; field names shared with
/// `CoworkLibraryArtifactRecord`.
#[derive(Debug, Deserialize)]
pub struct StoredArtifactSummary {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    // Why: absent on a record written before the manifest carried ownership;
    // such a record renders ungrouped until the next sync rewrites it.
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[must_use]
pub fn read_library_store(target_dir: &Path) -> BTreeMap<String, StoredArtifactSummary> {
    let Some(store) = std::fs::read(target_dir.join(LIBRARY_STORE_FILE))
        .ok()
        .and_then(|bytes| {
            serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(&bytes).ok()
        })
    else {
        return BTreeMap::new();
    };
    store
        .into_iter()
        .filter_map(|(id, value)| {
            serde_json::from_value::<StoredArtifactSummary>(value)
                .ok()
                .map(|summary| (id, summary))
        })
        .collect()
}

pub trait ArtifactSink: Send + Sync {
    fn is_materialized(&self, target_dir: &Path) -> bool;

    // Why: this is not `is_materialized` because the version stamp in
    // `version.json` hashes only the ids the manifest carries, so it matches
    // even when the sink is holding extra records the manifest has since
    // dropped. An install that accumulated stale ids before the sinks became
    // authoritative would take the "up to date, skipping" path forever and
    // never shed them. Reporting the extras as not-current is what makes the
    // next sync repair the store.
    fn is_current(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> bool;

    // Why: authoritative, mirroring `sync::apply::plugin::remove_stale`:
    // anything the sink holds that `artifacts` does not name is removed.
    // `artifacts` is never empty — `emit::write_artifacts` returns early on an
    // empty set so a transient empty manifest cannot wipe the store.
    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError>;
}

fn expected_ids(artifacts: &[ArtifactEntry]) -> BTreeSet<&str> {
    artifacts.iter().map(|a| a.id.as_str()).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct FileSink;

impl ArtifactSink for FileSink {
    fn is_materialized(&self, target_dir: &Path) -> bool {
        target_dir.join(LIBRARY_STORE_FILE).is_file()
    }

    fn is_current(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> bool {
        let stored: BTreeSet<String> = read_library_store(target_dir).into_keys().collect();
        let expected = expected_ids(artifacts);
        stored.len() == expected.len() && stored.iter().all(|id| expected.contains(id.as_str()))
    }

    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
        let path = target_dir.join(LIBRARY_STORE_FILE);
        let previous: BTreeSet<String> = read_library_store(target_dir).into_keys().collect();
        let expected = expected_ids(artifacts);
        let dropped: Vec<&str> = previous
            .iter()
            .map(String::as_str)
            .filter(|id| !expected.contains(id))
            .collect();
        if !dropped.is_empty() {
            tracing::info!(
                removed = ?dropped,
                target_dir = %target_dir.display(),
                "cowork artifacts: dropping records the manifest no longer carries"
            );
        }
        // Why: built fresh rather than merged into the previous map: the store is a
        // projection of the manifest, so a key absent from `artifacts` is gone.
        let mut store = serde_json::Map::new();
        for artifact in artifacts {
            let record = serde_json::to_value(CoworkLibraryArtifactRecord::from(artifact))
                .map_err(|e| ApplyError::Serialize {
                    what: "artifact record".into(),
                    source: e,
                })?;
            store.insert(artifact.id.as_str().to_owned(), record);
        }
        let bytes = serde_json::to_vec_pretty(&store).map_err(|e| ApplyError::Serialize {
            what: LIBRARY_STORE_FILE.into(),
            source: e,
        })?;
        crate::fsutil::atomic_write_0600(&path, &bytes).map_err(|e| ApplyError::Io {
            context: format!("write {}", path.display()),
            source: e,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SeedStaging;

impl ArtifactSink for SeedStaging {
    fn is_materialized(&self, target_dir: &Path) -> bool {
        target_dir.join(STAGING_SUBDIR).is_dir()
    }

    fn is_current(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> bool {
        let dir = target_dir.join(STAGING_SUBDIR);
        let staged = staged_ids(&dir);
        let expected = expected_ids(artifacts);
        // Why: only ids that survive `safe_id_segment` are ever staged, so an
        // unsafe id must not count as missing here.
        staged.len() == expected.iter().filter(|id| safe_id_segment(id)).count()
            && staged.iter().all(|id| expected.contains(id.as_str()))
    }

    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
        let dir = target_dir.join(STAGING_SUBDIR);
        // Why: the seed skill copies whatever is staged into the library. A
        // record left here from a manifest that no longer names it would
        // re-introduce the very id `FileSink` just dropped.
        prune_staging(&dir, &expected_ids(artifacts))?;
        for artifact in artifacts {
            let id = artifact.id.as_str();
            if !safe_id_segment(id) {
                tracing::warn!(
                    artifact_id = %id,
                    "cowork artifacts: unsafe artifact id for staging filename; skipping"
                );
                continue;
            }
            let bytes = serde_json::to_vec_pretty(&CoworkLibraryArtifactRecord::from(artifact))
                .map_err(|e| ApplyError::Serialize {
                    what: "artifact record".into(),
                    source: e,
                })?;
            let path = dir.join(format!("{id}.json"));
            crate::fsutil::atomic_write_0600(&path, &bytes).map_err(|e| ApplyError::Io {
                context: format!("write {}", path.display()),
                source: e,
            })?;
        }
        Ok(())
    }
}

fn staged_ids(dir: &Path) -> BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                return None;
            }
            path.file_stem().and_then(|s| s.to_str()).map(str::to_owned)
        })
        .collect()
}

fn prune_staging(dir: &Path, expected: &BTreeSet<&str>) -> Result<(), ApplyError> {
    for id in staged_ids(dir) {
        if expected.contains(id.as_str()) {
            continue;
        }
        let path = dir.join(format!("{id}.json"));
        std::fs::remove_file(&path).map_err(|e| ApplyError::Io {
            context: format!("remove stale staged artifact {}", path.display()),
            source: e,
        })?;
        tracing::info!(
            artifact_id = %id,
            "cowork artifacts: removed staged record the manifest no longer carries"
        );
    }
    Ok(())
}
