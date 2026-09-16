//! Repository assembly of verified revision closures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use systemprompt_identifiers::{ResourceRevisionId, UserId};
use systemprompt_models::managed::{
    ASSEMBLER_VERSION, AssetDigest, MAX_BYTES, MAX_FILES, MAX_REVISIONS, RevisionBundle,
};

use super::error::invalid;
use super::{ManagedError, ManagedRepository, Result};

impl ManagedRepository {
    pub async fn get_revision_bundle(
        &self,
        owner: &UserId,
        root: &ResourceRevisionId,
    ) -> Result<RevisionBundle> {
        let mut bundle = RevisionBundle {
            schema_version: 1,
            assembler_version: ASSEMBLER_VERSION.to_owned(),
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
