//! Pluggable write targets for Cowork library artifacts.
//!
//! The live Cowork library ingests artifacts only via its native
//! `create_artifact` tool, so [`super::emit::active_sinks`] writes through
//! both implementations of [`ArtifactSink`]:
//!
//! - [`FileSink`] writes/merges an on-disk store keyed by artifact id (read by
//!   the bridge GUI's Artifacts listing; usable directly if the library ever
//!   becomes file-writable).
//! - [`SeedStaging`] drops one record per artifact into a staging dir for the
//!   first-run `create_artifact` seed skill to consume.
//!
//! The record shape mirrors Cowork's `create_artifact` input. Store paths and
//! field names live only here — writers use `LibraryArtifactRecord`, readers
//! (the GUI listing) use [`StoredArtifactSummary`] via [`read_library_store`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::gateway::manifest::ArtifactEntry;
use crate::sync::apply::ApplyError;
use crate::sync::hash::safe_id_segment;

pub const LIBRARY_STORE_FILE: &str = "library.json";

pub const STAGING_SUBDIR: &str = "staging";

/// Field names track Cowork's native library-entry shape.
#[derive(Debug, Serialize)]
struct LibraryArtifactRecord<'a> {
    id: &'a str,
    name: &'a str,
    description: &'a str,
    version: &'a str,
    content: &'a str,
    #[serde(rename = "isStarred")]
    is_starred: bool,
    #[serde(rename = "mcpTools")]
    mcp_tools: &'a [String],
}

impl<'a> From<&'a ArtifactEntry> for LibraryArtifactRecord<'a> {
    fn from(a: &'a ArtifactEntry) -> Self {
        Self {
            id: a.id.as_str(),
            name: &a.name,
            description: &a.description,
            version: &a.version,
            content: &a.content,
            is_starred: a.starred,
            mcp_tools: &a.mcp_tools,
        }
    }
}

/// Read-side view of a store entry; field names shared with
/// `LibraryArtifactRecord`.
#[derive(Debug, Deserialize)]
pub struct StoredArtifactSummary {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Missing or unreadable store yields an empty map; foreign entries (preserved
/// verbatim on write) that don't match the record shape are skipped.
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
    /// Guards the idempotency check against a half-written or externally
    /// removed store that the `version.json` match alone would skip.
    fn is_materialized(&self, target_dir: &Path) -> bool;

    /// Entries in the store not owned by this sync are preserved.
    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError>;
}

#[derive(Debug, Clone, Copy)]
pub struct FileSink;

impl ArtifactSink for FileSink {
    fn is_materialized(&self, target_dir: &Path) -> bool {
        target_dir.join(LIBRARY_STORE_FILE).is_file()
    }

    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
        let path = target_dir.join(LIBRARY_STORE_FILE);
        let mut store: serde_json::Map<String, serde_json::Value> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        for artifact in artifacts {
            let record =
                serde_json::to_value(LibraryArtifactRecord::from(artifact)).map_err(|e| {
                    ApplyError::Serialize {
                        what: "artifact record".into(),
                        source: e,
                    }
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

    fn write(&self, target_dir: &Path, artifacts: &[ArtifactEntry]) -> Result<(), ApplyError> {
        let dir = target_dir.join(STAGING_SUBDIR);
        for artifact in artifacts {
            let id = artifact.id.as_str();
            if !safe_id_segment(id) {
                tracing::warn!(
                    artifact_id = %id,
                    "cowork artifacts: unsafe artifact id for staging filename; skipping"
                );
                continue;
            }
            let bytes =
                serde_json::to_vec_pretty(&LibraryArtifactRecord::from(artifact)).map_err(|e| {
                    ApplyError::Serialize {
                        what: "artifact record".into(),
                        source: e,
                    }
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
