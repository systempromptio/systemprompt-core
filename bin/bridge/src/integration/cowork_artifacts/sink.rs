//! Pluggable write targets for Cowork library artifacts.
//!
//! The Cowork global Artifacts-library store shape is not yet confirmed on a
//! live build (see the module head in [`super`]), so the write mechanism is
//! kept behind [`ArtifactSink`] with two implementations selected at compile
//! time by [`super::emit::active_sink`]:
//!
//! - [`FileSink`] writes/merges an on-disk store keyed by artifact id (used if
//!   the library is directly file-writable).
//! - [`SeedStaging`] drops one record per artifact into a staging dir for a
//!   first-run `create_artifact` seed skill to consume (used if the library is
//!   `create_artifact`-only).
//!
//! Only the record field names and store path are expected to change once the
//! live schema is confirmed — they are the [`LIBRARY_STORE_FILE`] /
//! [`STAGING_SUBDIR`] / `LibraryArtifactRecord` constants below.

use std::path::Path;

use serde::Serialize;

use crate::gateway::manifest::ArtifactEntry;
use crate::sync::apply::ApplyError;
use crate::sync::hash::safe_id_segment;

/// Best-guess pending the live-Cowork schema.
pub const LIBRARY_STORE_FILE: &str = "library.json";

pub const STAGING_SUBDIR: &str = "staging";

/// Field names track Cowork's native library-entry shape; reconcile against a
/// live entry before shipping.
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
