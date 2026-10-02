//! Device-authenticated consumer evidence and correctable attribution.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::receipts::{ConsumerReceiptRequest, InstallationPlanFile};

use super::plan::ships;
use crate::managed::{AssetDigest, InstalledFile, ManagedError, Result, RevisionBundle};

pub fn verify_readback(bundle: &RevisionBundle, request: &ConsumerReceiptRequest) -> Result<()> {
    request
        .validate()
        .map_err(|error| crate::managed::error::invalid_input("consumer receipt request", error))?;
    bundle.verify()?;
    if bundle.root != request.revision_id
        || bundle.digest()?.as_str() != request.bundle_digest.as_str()
    {
        return Err(ManagedError::Integrity);
    }
    let expected_count: usize = bundle
        .revisions
        .values()
        .map(|revision| revision.files.keys().filter(|path| ships(path)).count())
        .sum();
    if expected_count != request.files.len() {
        return Err(ManagedError::Integrity);
    }
    for file in &request.files {
        let expected = bundle
            .revisions
            .get(&file.revision_id)
            .and_then(|revision| revision.files.get(&file.path))
            .filter(|_| ships(&file.path))
            .ok_or(ManagedError::Integrity)?;
        if file.digest.as_str() != expected.digest.as_str()
            || file.bytes != expected.bytes
            || file.executable != expected.executable
        {
            return Err(ManagedError::Integrity);
        }
    }
    Ok(())
}

pub(super) fn verify_runtime_files(
    request: &ConsumerReceiptRequest,
    expected_runtime: &[InstallationPlanFile],
) -> Result<()> {
    if request.runtime_files.is_empty() {
        return Ok(());
    }
    if request.runtime_files.len() != expected_runtime.len() {
        return Err(ManagedError::Integrity);
    }
    for expected in expected_runtime {
        let actual = request
            .runtime_files
            .iter()
            .find(|file| file.path == expected.path)
            .ok_or(ManagedError::Integrity)?;
        if actual.digest != ContentDigest::of(&expected.bytes)
            || actual.bytes != expected.bytes.len() as u64
            || actual.executable != expected.executable
        {
            return Err(ManagedError::Integrity);
        }
    }
    Ok(())
}

pub(super) fn installed_files(request: &ConsumerReceiptRequest) -> Result<Vec<InstalledFile>> {
    request
        .files
        .iter()
        .map(|file| {
            Ok(InstalledFile {
                revision_id: file.revision_id.clone(),
                path: file.path.clone(),
                digest: AssetDigest::try_from(file.digest.as_str().to_owned())?,
                bytes: file.bytes,
                executable: file.executable,
            })
        })
        .collect()
}
