//! Deterministic, verified revision closures for evaluator and distribution
//! consumers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::error::invalid;
use super::{
    AssetDigest, AssetFile, ManagedError, ManagedRepository, Result, RevisionFiles,
    RevisionManifest,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use systemprompt_identifiers::{ResourceRevisionId, UserId};

const MAX_REVISIONS: usize = 64;
const MAX_FILES: usize = 256;
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// The wire form is untrusted until `verify` succeeds. Hashes prove integrity,
/// not authorization; repository resolution checks ownership independently.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionBundle {
    pub schema_version: u32,
    pub root: ResourceRevisionId,
    pub revisions: BTreeMap<ResourceRevisionId, RevisionManifest>,
    pub assets: BTreeMap<AssetDigest, Vec<u8>>,
}

impl RevisionBundle {
    pub fn verify(&self) -> Result<()> {
        if self.schema_version != 1
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
                    .ok_or(ManagedError::Integrity)?;
                expanded_bytes = expanded_bytes
                    .checked_add(bytes.len())
                    .ok_or(ManagedError::Integrity)?;
                if expanded_bytes > MAX_BYTES {
                    return Err(invalid("Bundle exceeds 8 MiB expanded content"));
                }
                if bytes.len() as u64 != entry.bytes || AssetDigest::of(bytes) != entry.digest {
                    return Err(ManagedError::Integrity);
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
                return Err(ManagedError::Integrity);
            }
        }
        if used_assets.len() != self.assets.len() {
            return Err(ManagedError::Integrity);
        }
        let mut visited = BTreeSet::new();
        self.visit(&self.root, &mut BTreeSet::new(), &mut visited)?;
        if visited.len() != self.revisions.len() {
            return Err(ManagedError::Integrity);
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
        let manifest = self.revisions.get(id).ok_or(ManagedError::Integrity)?;
        for dependency in manifest.dependencies.values() {
            let target = self
                .revisions
                .get(&dependency.revision_id)
                .ok_or(ManagedError::Integrity)?;
            if target.digest()? != dependency.digest {
                return Err(ManagedError::Integrity);
            }
            self.visit(&dependency.revision_id, active, visited)?;
        }
        active.remove(id);
        visited.insert(id.clone());
        Ok(())
    }

    /// Canonical bytes bind the exact dependency closure, file bytes, media
    /// types and executable modes. No filesystem read or timestamp enters
    /// this value.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.verify()?;
        Ok(serde_jcs::to_vec(self)?)
    }

    pub fn digest(&self) -> Result<AssetDigest> {
        Ok(AssetDigest::of(&self.canonical_bytes()?))
    }

    pub fn revision_files(&self, id: &ResourceRevisionId) -> Result<RevisionFiles> {
        self.verify()?;
        let manifest = self.revisions.get(id).ok_or(ManagedError::Unavailable)?;
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

impl ManagedRepository {
    /// Resolve only immutable, owned records. Parent history is provenance, not
    /// a runtime dependency; only declared dependency edges enter the closure.
    pub async fn get_revision_bundle(
        &self,
        owner: &UserId,
        root: &ResourceRevisionId,
    ) -> Result<RevisionBundle> {
        let mut bundle = RevisionBundle {
            schema_version: 1,
            root: root.clone(),
            revisions: BTreeMap::new(),
            assets: BTreeMap::new(),
        };
        let mut pending = vec![root.clone()];
        let mut file_count = 0usize;
        let mut expanded_bytes = 0usize;
        while let Some(id) = pending.pop() {
            if bundle.revisions.contains_key(&id) {
                continue;
            }
            if bundle.revisions.len() >= MAX_REVISIONS {
                return Err(invalid("Bundle exceeds 64 revisions"));
            }
            let manifest = self.get_revision(owner, &id).await?;
            let files = self.get_revision_files(owner, &id).await?;
            file_count += files.0.len();
            if file_count > MAX_FILES {
                return Err(invalid("Bundle exceeds 256 files"));
            }
            for file in files.0.values() {
                expanded_bytes = expanded_bytes
                    .checked_add(file.bytes.len())
                    .ok_or(ManagedError::Integrity)?;
                if expanded_bytes > MAX_BYTES {
                    return Err(invalid("Bundle exceeds 8 MiB expanded content"));
                }
                bundle
                    .assets
                    .entry(AssetDigest::of(&file.bytes))
                    .or_insert_with(|| file.bytes.clone());
            }
            pending.extend(
                manifest
                    .dependencies
                    .values()
                    .map(|dependency| dependency.revision_id.clone()),
            );
            bundle.revisions.insert(id, manifest);
        }
        bundle.verify()?;
        Ok(bundle)
    }
}
