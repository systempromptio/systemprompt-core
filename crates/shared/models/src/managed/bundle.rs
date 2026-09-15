//! Deterministic, verified revision closures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_identifiers::ResourceRevisionId;

use super::assets::{AssetDigest, AssetFile, RevisionFiles};
use super::error::{RevisionBundleError, invalid};
use super::manifest::RevisionManifest;

type Result<T> = std::result::Result<T, RevisionBundleError>;

pub const MAX_REVISIONS: usize = 64;
pub const MAX_FILES: usize = 256;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const ASSEMBLER_VERSION: &str = "managed-bundle-v1";

/// A revision closure whose wire form is untrusted until `verify` succeeds.
///
/// Only declared dependencies enter the closure; parent revision history
/// remains provenance.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RevisionBundle {
    pub schema_version: u32,
    pub assembler_version: String,
    pub root: ResourceRevisionId,
    pub revisions: BTreeMap<ResourceRevisionId, RevisionManifest>,
    pub assets: BTreeMap<AssetDigest, Vec<u8>>,
}

impl std::fmt::Debug for RevisionBundle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionBundle")
            .field("schema_version", &self.schema_version)
            .field("root", &self.root)
            .field("revisions", &self.revisions.len())
            .field("assets", &self.assets.len())
            .finish_non_exhaustive()
    }
}

impl RevisionBundle {
    pub fn verify(&self) -> Result<()> {
        if self.schema_version != 1
            || self.assembler_version != ASSEMBLER_VERSION
            || self.revisions.is_empty()
            || self.revisions.len() > MAX_REVISIONS
        {
            return Err(invalid("Unsupported or excessive revision bundle"));
        }
        let mut used_assets = BTreeSet::new();
        let mut file_count = 0usize;
        let mut expanded_bytes = 0usize;
        for manifest in self.revisions.values() {
            if manifest.schema_version != 1 {
                return Err(invalid("Unsupported revision manifest"));
            }
            file_count += manifest.files.len();
            if file_count > MAX_FILES {
                return Err(invalid("Bundle exceeds 256 files"));
            }
            let mut files = RevisionFiles::default();
            for (path, entry) in &manifest.files {
                let bytes = self
                    .assets
                    .get(&entry.digest)
                    .ok_or(RevisionBundleError::Integrity)?;
                expanded_bytes = expanded_bytes
                    .checked_add(bytes.len())
                    .ok_or(RevisionBundleError::Integrity)?;
                if expanded_bytes > MAX_BYTES {
                    return Err(invalid("Bundle exceeds 8 MiB expanded content"));
                }
                if bytes.len() as u64 != entry.bytes || AssetDigest::of(bytes) != entry.digest {
                    return Err(RevisionBundleError::Integrity);
                }
                used_assets.insert(entry.digest.clone());
                files.0.insert(
                    path.clone(),
                    AssetFile {
                        bytes: bytes.clone(),
                        media_type: entry.media_type.clone(),
                        executable: entry.executable,
                    },
                );
            }
            let rebuilt = RevisionManifest::from_files(
                manifest.snapshot_id.clone(),
                manifest.parent_id.clone(),
                &files,
                manifest.dependencies.clone(),
            )?;
            if &rebuilt != manifest {
                return Err(RevisionBundleError::Integrity);
            }
        }
        if used_assets.len() != self.assets.len() {
            return Err(RevisionBundleError::Integrity);
        }
        let mut visited = BTreeSet::new();
        self.visit(&self.root, &mut BTreeSet::new(), &mut visited)?;
        if visited.len() != self.revisions.len() {
            return Err(RevisionBundleError::Integrity);
        }
        Ok(())
    }

    fn visit(
        &self,
        id: &ResourceRevisionId,
        active: &mut BTreeSet<ResourceRevisionId>,
        visited: &mut BTreeSet<ResourceRevisionId>,
    ) -> Result<()> {
        if visited.contains(id) {
            return Ok(());
        }
        if !active.insert(id.clone()) {
            return Err(invalid("Cyclic revision dependencies"));
        }
        let manifest = self
            .revisions
            .get(id)
            .ok_or(RevisionBundleError::Integrity)?;
        for dependency in manifest.dependencies.values() {
            let target = self
                .revisions
                .get(&dependency.revision_id)
                .ok_or(RevisionBundleError::Integrity)?;
            if target.digest()? != dependency.digest {
                return Err(RevisionBundleError::Integrity);
            }
            self.visit(&dependency.revision_id, active, visited)?;
        }
        active.remove(id);
        visited.insert(id.clone());
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.verify()?;
        Ok(serde_jcs::to_vec(self)?)
    }

    pub fn digest(&self) -> Result<AssetDigest> {
        Ok(AssetDigest::of(&self.canonical_bytes()?))
    }

    pub fn revision_files(&self, id: &ResourceRevisionId) -> Result<RevisionFiles> {
        self.verify()?;
        let manifest = self
            .revisions
            .get(id)
            .ok_or_else(|| RevisionBundleError::MissingRevision(id.clone()))?;
        Ok(RevisionFiles(
            manifest
                .files
                .iter()
                .map(|(path, entry)| {
                    (
                        path.clone(),
                        AssetFile {
                            bytes: self.assets[&entry.digest].clone(),
                            media_type: entry.media_type.clone(),
                            executable: entry.executable,
                        },
                    )
                })
                .collect(),
        ))
    }
}
