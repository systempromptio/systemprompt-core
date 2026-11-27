//! The installation receipt the device builds from a consumer plan and what it
//! read back from disk.
//!
//! The plan, the readback entries and the receipt are `systemprompt-models`'
//! own types, the ones the instance validates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::ConsumerInstallationId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::ManagedPublicationRef;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptRequest, FileReadback, ReadbackStatus,
    RuntimeFileReadback,
};

use crate::protocol::names::sha256_hex;

#[must_use]
pub fn plan_matches(
    plan: &ConsumerInstallationPlan,
    publication: &ManagedPublicationRef,
    host: HostKind,
) -> bool {
    plan.host.accepts(host)
        && plan.publication_id == publication.publication_id
        && plan.resource_id == publication.resource_id
        && plan.revision_id == publication.revision_id
        && plan.generation == publication.generation
        && plan.bundle_digest.as_str() == publication.bundle_digest.as_str()
}

/// Why a readback does not make a receipt: the path that does not hold.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("runtime file {path} does not match the plan")]
pub struct ReadbackMismatch {
    pub path: String,
}

pub fn build_receipt(
    plan: &ConsumerInstallationPlan,
    readback: &[RuntimeFileReadback],
    installation_id: ConsumerInstallationId,
    observed_at: DateTime<Utc>,
) -> Result<ConsumerReceiptRequest, ReadbackMismatch> {
    let mut runtime = Vec::with_capacity(plan.runtime_files.len());
    for file in &plan.runtime_files {
        let digest = sha256_hex(&file.bytes);
        let size = u64::try_from(file.bytes.len()).unwrap_or(u64::MAX);
        let found = readback
            .iter()
            .find(|entry| entry.path == file.path)
            .filter(|found| {
                found.content_check == ReadbackStatus::Verified
                    && found.mode_check != ReadbackStatus::Mismatch
                    && found.digest.as_str() == digest
                    && found.bytes == size
                    && found.executable == file.executable
            });
        let Some(found) = found else {
            return Err(ReadbackMismatch {
                path: file.path.clone(),
            });
        };
        runtime.push(found.clone());
    }
    let mut files = Vec::with_capacity(plan.canonical_files.len());
    for file in &plan.canonical_files {
        let copy_path = format!(
            ".systemprompt-source/{}/{}",
            file.revision_id.as_str(),
            file.path
        );
        let copy = runtime
            .iter()
            .find(|entry| entry.path == copy_path)
            .filter(|copy| {
                copy.digest == file.digest
                    && copy.bytes == file.bytes
                    && copy.executable == file.executable
            });
        let Some(copy) = copy else {
            return Err(ReadbackMismatch { path: copy_path });
        };
        files.push(FileReadback {
            content_check: copy.content_check,
            mode_check: copy.mode_check,
            ..file.clone()
        });
    }
    files.sort_by(|a, b| {
        (a.revision_id.as_str(), a.path.as_str()).cmp(&(b.revision_id.as_str(), b.path.as_str()))
    });
    runtime.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ConsumerReceiptRequest {
        installation_id,
        publication_id: plan.publication_id.clone(),
        resource_id: plan.resource_id.clone(),
        revision_id: plan.revision_id.clone(),
        generation: plan.generation,
        bundle_digest: plan.bundle_digest.clone(),
        host: plan.host.clone(),
        observed_at,
        files,
        runtime_files: runtime,
    })
}
