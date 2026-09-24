//! Persistent device-authenticated installation feedback.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackError, ReadbackFault, Result};
use chrono::Utc;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use systemprompt_identifiers::ConsumerInstallationId;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptRequest, ReadbackStatus, RuntimeFileReadback,
};
use systemprompt_models::feedback::{ContentDigest, validate_relative_path};

pub fn materialize(root: &Path, plan: &ConsumerInstallationPlan) -> Result<()> {
    validate_plan(plan)?;
    if !root.join("SKILL.md").is_file() {
        return Err(FeedbackError::Readback(ReadbackFault::SkillMissing));
    }
    let sidecar = root.join(".systemprompt-runtime.json");
    let previous: Vec<String> = if sidecar.exists() {
        if std::fs::metadata(&sidecar)?.len() > 1024 * 1024 {
            return Err(FeedbackError::Readback(ReadbackFault::SidecarOversized));
        }
        serde_json::from_slice(&std::fs::read(&sidecar)?)?
    } else {
        Vec::new()
    };
    for file in &plan.runtime_files {
        let path = safe_target(root, &file.path)?;
        crate::fsutil::atomic_write_0644(&path, &file.bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &path,
                std::fs::Permissions::from_mode(if file.executable { 0o755 } else { 0o644 }),
            )?;
        }
    }
    for stale in previous
        .iter()
        .filter(|path| !plan.runtime_files.iter().any(|file| &file.path == *path))
    {
        let path = safe_target(root, stale)?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    let paths: Vec<_> = plan
        .runtime_files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    crate::fsutil::atomic_write_0600(&sidecar, &serde_json::to_vec(&paths)?)?;
    Ok(())
}

pub fn verify(
    root: &Path,
    plan: &ConsumerInstallationPlan,
    installation_id: ConsumerInstallationId,
) -> Result<ConsumerReceiptRequest> {
    validate_plan(plan)?;
    let mut runtime_files = Vec::new();
    for expected in &plan.runtime_files {
        let path = safe_target(root, &expected.path)?;
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_file()
            || metadata.len() != expected.bytes.len() as u64
            || std::fs::read(&path)? != expected.bytes
        {
            return Err(FeedbackError::Readback(ReadbackFault::ContentMismatch(
                expected.path.clone(),
            )));
        }
        let bytes = expected.bytes.clone();
        #[cfg(unix)]
        let mode_check = verify_mode(&metadata, expected.executable, &expected.path)?;
        // Why: no mode bits to read here; a plain file has nothing to verify
        // and an executable cannot be confirmed.
        #[cfg(not(unix))]
        let mode_check = if expected.executable {
            ReadbackStatus::Unavailable
        } else {
            ReadbackStatus::Verified
        };
        runtime_files.push(RuntimeFileReadback {
            path: expected.path.clone(),
            digest: ContentDigest::of(&bytes),
            bytes: bytes.len() as u64,
            executable: expected.executable,
            content_check: ReadbackStatus::Verified,
            mode_check,
        });
    }
    let mut files = plan.canonical_files.clone();
    for canonical in &mut files {
        let path = format!(
            ".systemprompt-source/{}/{}",
            canonical.revision_id, canonical.path
        );
        let actual = runtime_files
            .iter()
            .find(|file| file.path == path)
            .ok_or_else(|| {
                FeedbackError::Readback(ReadbackFault::CanonicalMissing(path.clone()))
            })?;
        if actual.digest != canonical.digest
            || actual.bytes != canonical.bytes
            || actual.executable != canonical.executable
        {
            return Err(FeedbackError::Readback(ReadbackFault::ContentMismatch(
                path,
            )));
        }
        canonical.content_check = actual.content_check;
        canonical.mode_check = actual.mode_check;
    }
    Ok(ConsumerReceiptRequest {
        installation_id,
        publication_id: plan.publication_id.clone(),
        resource_id: plan.resource_id.clone(),
        revision_id: plan.revision_id.clone(),
        generation: plan.generation,
        bundle_digest: plan.bundle_digest.clone(),
        host: plan.host,
        observed_at: Utc::now(),
        files,
        runtime_files,
    })
}

fn validate_plan(plan: &ConsumerInstallationPlan) -> Result<()> {
    if plan.runtime_files.is_empty()
        || plan.runtime_files.len() > 8192
        || plan.canonical_files.len() > 4096
    {
        return Err(FeedbackError::Readback(ReadbackFault::PlanInvalid));
    }
    let mut seen = BTreeSet::new();
    let mut bytes = 0usize;
    for file in &plan.runtime_files {
        validate_relative_path(&file.path)?;
        bytes = bytes.saturating_add(file.bytes.len());
        if !seen.insert(&file.path) || bytes > 24 * 1024 * 1024 {
            return Err(FeedbackError::Readback(ReadbackFault::PlanInvalid));
        }
    }
    if !seen.contains(&"SKILL.md".to_owned()) {
        return Err(FeedbackError::Readback(ReadbackFault::PlanInvalid));
    }
    Ok(())
}

fn safe_target(root: &Path, relative: &str) -> Result<PathBuf> {
    validate_relative_path(relative)?;
    let mut path = root.to_path_buf();
    reject_link(&path)?;
    for component in relative.split('/') {
        path.push(component);
        reject_link(&path)?;
    }
    Ok(path)
}

fn reject_link(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_symlink() {
                return Err(unsafe_path(path));
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(unsafe_path(path));
                }
            }
            Ok(())
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn unsafe_path(path: &Path) -> FeedbackError {
    FeedbackError::Readback(ReadbackFault::UnsafePath(path.display().to_string()))
}

#[cfg(unix)]
fn verify_mode(metadata: &std::fs::Metadata, expected: bool, path: &str) -> Result<ReadbackStatus> {
    use std::os::unix::fs::PermissionsExt;
    if (metadata.permissions().mode() & 0o7777) != if expected { 0o755 } else { 0o644 } {
        return Err(FeedbackError::Readback(ReadbackFault::ModeMismatch(
            path.to_owned(),
        )));
    }
    Ok(ReadbackStatus::Verified)
}
