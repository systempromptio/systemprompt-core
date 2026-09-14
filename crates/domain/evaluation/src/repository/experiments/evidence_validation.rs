//! Managed workspace and execution evidence validation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{EvidenceArchive, ExecutionEvidence, Result, VariantSpec, conflict, invalid};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ManagedAsset {
    pub path: String,
    pub digest: String,
    pub content: Vec<u8>,
    pub executable: bool,
}

pub(super) fn managed_assets(manifest: &serde_json::Value) -> Result<Vec<ManagedAsset>> {
    let revisions = manifest
        .get("revisions")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("Managed workspace requires revision manifests"))?;
    let assets = manifest
        .get("assets")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid("Managed workspace requires exact assets"))?;
    let mut rows = Vec::new();
    for (revision, value) in revisions {
        let files = value
            .get("files")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| invalid("Managed revision files are missing"))?;
        for (path, entry) in files {
            if path.starts_with('/')
                || path.contains(['\\', ':'])
                || path.split('/').any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(invalid("Managed workspace contains a non-portable path"));
            }
            let digest = entry
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| invalid("Managed file digest is missing"))?;
            let bytes: Vec<u8> = serde_json::from_value(
                assets
                    .get(digest)
                    .cloned()
                    .ok_or_else(|| invalid("Managed file asset is missing"))?,
            )?;
            let declared = entry
                .get("bytes")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| invalid("Managed file byte count is missing"))?;
            let executable = entry
                .get("executable")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| invalid("Managed file mode is missing"))?;
            if bytes.len() as u64 != declared || hex::encode(Sha256::digest(&bytes)) != digest {
                return Err(invalid(
                    "Managed file bytes differ from their digest or length",
                ));
            }
            rows.push(ManagedAsset {
                path: format!("{revision}/{path}"),
                digest: digest.to_owned(),
                content: bytes,
                executable,
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
    if evidence.candidate_bundle_digest != variant.skill_bundle_digest
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
