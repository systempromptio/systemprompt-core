//! Canonical revision manifests bind files, dependencies and provenance.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_identifiers::{ResourceRevisionId, SourceSnapshotId};

use super::error::invalid;
use super::{AssetDigest, Result, RevisionFiles};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub digest: AssetDigest,
    pub bytes: u64,
    pub media_type: String,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyRef {
    pub revision_id: ResourceRevisionId,
    pub digest: AssetDigest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionManifest {
    pub schema_version: u32,
    pub snapshot_id: SourceSnapshotId,
    pub parent_id: Option<ResourceRevisionId>,
    pub files: BTreeMap<String, FileEntry>,
    pub dependencies: BTreeMap<String, DependencyRef>,
}

impl RevisionManifest {
    pub fn from_files(
        snapshot_id: SourceSnapshotId,
        parent_id: Option<ResourceRevisionId>,
        files: &RevisionFiles,
        dependencies: BTreeMap<String, DependencyRef>,
    ) -> Result<Self> {
        files.validate()?;
        if dependencies.len() > 256 {
            return Err(invalid("Revision has more than 256 dependencies"));
        }
        let mut ids = BTreeSet::new();
        for (key, dependency) in &dependencies {
            super::provenance::validate_key(key)?;
            if !ids.insert(dependency.revision_id.as_str()) {
                return Err(invalid("Duplicate dependency revision"));
            }
        }
        Ok(Self {
            schema_version: 1,
            snapshot_id,
            parent_id,
            files: files
                .0
                .iter()
                .map(|(path, file)| {
                    (
                        path.clone(),
                        FileEntry {
                            digest: AssetDigest::of(&file.bytes),
                            bytes: file.bytes.len() as u64,
                            media_type: file.media_type.clone(),
                            executable: file.executable,
                        },
                    )
                })
                .collect(),
            dependencies,
        })
    }

    pub fn digest(&self) -> Result<AssetDigest> {
        Ok(AssetDigest::of(&serde_jcs::to_vec(self)?))
    }
}
