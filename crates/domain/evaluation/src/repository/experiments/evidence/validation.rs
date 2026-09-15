//! Managed workspace and execution evidence validation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{EvidenceArchive, ExecutionEvidence, Result, VariantSpec, conflict, invalid};
use sha2::{Digest, Sha256};
use systemprompt_models::managed::RevisionBundle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ManagedAsset {
    pub path: String,
    pub digest: String,
    pub content: Vec<u8>,
    pub executable: bool,
}

pub(super) fn managed_assets(bundle: &RevisionBundle) -> Result<Vec<ManagedAsset>> {
    bundle
        .verify()
        .map_err(|error| invalid(&format!("Managed workspace bundle rejected: {error}")))?;
    let mut rows = Vec::new();
    for (revision, manifest) in &bundle.revisions {
        for (path, entry) in &manifest.files {
            let content = bundle
                .assets
                .get(&entry.digest)
                .ok_or_else(|| invalid("Managed file asset is missing"))?;
            rows.push(ManagedAsset {
                path: format!("{}/{path}", revision.as_str()),
                digest: entry.digest.as_str().to_owned(),
                content: content.clone(),
                executable: entry.executable,
            });
        }
    }
    Ok(rows)
}

pub(super) fn validate_artifacts(
    evidence: &ExecutionEvidence,
    artifacts: &EvidenceArchive,
) -> Result<()> {
    artifacts.validate()?;
    if artifacts.files.len() != evidence.artifacts.len() {
        return Err(invalid("Artifact payload differs from its manifest"));
    }
    for artifact in &evidence.artifacts {
        let bytes = artifacts
            .files
            .get(&artifact.relative_path)
            .ok_or_else(|| invalid("Manifest artifact payload missing"))?
            .bytes
            .as_slice();
        if bytes.len() as u64 != artifact.bytes
            || hex::encode(Sha256::digest(bytes)) != artifact.sha256
        {
            return Err(invalid(
                "Artifact content does not match its declared hash or size",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_variant(evidence: &ExecutionEvidence, variant: &VariantSpec) -> Result<()> {
    if evidence.workspace_digest != variant.configuration_digest
        || evidence.candidate_bundle_digest != variant.skill_bundle_digest
        || evidence.installed_bundle_digest != variant.skill_bundle_digest
        || evidence.capabilities.image_digest != variant.worker_image_digest
        || evidence.capabilities.client != variant.client
        || evidence.capabilities.client_version != variant.client_version
    {
        return Err(conflict(
            "Evidence differs from the frozen experiment variant",
        ));
    }
    Ok(())
}
