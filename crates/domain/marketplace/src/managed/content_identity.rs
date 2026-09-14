//! Source-independent content identity retains dependency names, exact file
//! digests, media types and executable modes while excluding revision history.

use super::{AssetDigest, ManagedError, Result, RevisionBundle};
use serde::Serialize;
use std::collections::BTreeMap;
use systemprompt_identifiers::ResourceRevisionId;

#[derive(Serialize)]
struct ContentNode<'a> {
    files: &'a BTreeMap<String, super::FileEntry>,
    dependencies: BTreeMap<String, AssetDigest>,
}

impl RevisionBundle {
    pub fn content_digest(&self) -> Result<AssetDigest> {
        self.verify()?;
        self.node_content_digest(&self.root, 0, &mut BTreeMap::new())
    }

    fn node_content_digest(
        &self,
        id: &ResourceRevisionId,
        depth: usize,
        memo: &mut BTreeMap<ResourceRevisionId, AssetDigest>,
    ) -> Result<AssetDigest> {
        if depth > 64 {
            return Err(ManagedError::Integrity);
        }
        if let Some(digest) = memo.get(id) {
            return Ok(digest.clone());
        }
        let manifest = self.revisions.get(id).ok_or(ManagedError::Integrity)?;
        let dependencies = manifest
            .dependencies
            .iter()
            .map(|(key, dependency)| {
                Ok((
                    key.clone(),
                    self.node_content_digest(&dependency.revision_id, depth + 1, memo)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let digest = AssetDigest::of(&serde_jcs::to_vec(&ContentNode {
            files: &manifest.files,
            dependencies,
        })?);
        memo.insert(id.clone(), digest.clone());
        Ok(digest)
    }
}
