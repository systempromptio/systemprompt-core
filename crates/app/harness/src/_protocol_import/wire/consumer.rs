//! The device-authenticated consumer API that delivers a skill's runtime files.
//!
//! PROTOCOL section 10: device enrolment, the installation plan of a
//! publication, receipts and session bindings.
//!
//! Enrolment takes the user's JWT; every other call takes the enrolled device
//! credential. Consumer routes are rooted at the instance's origin and answer
//! errors as problem documents (`envelope::ProblemDocument`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use systemprompt_models::bridge::gateway::SelfEnrollRequest;

use crate::ids::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, InstallationSessionBindingId,
    ManagedResourceId, NativeSessionId, PublicationId, ResourceRevisionId, UserId,
};
use crate::names::sha256_hex;
use crate::secret::DeviceCredential;
use crate::wire::manifest::{Publication, Sha256Digest};

/// Body of `POST /v1/bridge/device` (`fingerprint` from
/// `names::device_fingerprint`, `label` naming this machine).
pub type DeviceEnrolRequest = SelfEnrollRequest;

/// Answer of `POST /v1/bridge/device`. The same fingerprint is the same
/// `device_id`; each call issues a new credential.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceEnrolment {
    pub device_id: DeviceId,
    pub consumer_id: UserId,
    pub credential: DeviceCredential,
}

/// How a file was checked when read back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    Verified,
    Mismatch,
    Unavailable,
}

/// A file of the publication's canonical revision, as the plan names it and the
/// receipt reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalFile {
    pub revision_id: ResourceRevisionId,
    pub path: String,
    pub digest: Sha256Digest,
    pub bytes: u64,
    pub executable: bool,
    pub content_check: Check,
    pub mode_check: Check,
}

/// A file to write beside the skill, with its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

/// Answer of `GET
/// /api/v1/consumer/resources/{resource}/publications/{publication}/bundle?
/// host=…`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationPlan {
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: Sha256Digest,
    pub host: String,
    #[serde(default)]
    pub canonical_files: Vec<CanonicalFile>,
    #[serde(default)]
    pub runtime_files: Vec<RuntimeFile>,
}

impl InstallationPlan {
    #[must_use]
    pub fn matches(&self, publication: &Publication, host: &str) -> bool {
        self.host == host
            && self.publication_id == publication.publication_id
            && self.resource_id == publication.resource_id
            && self.revision_id == publication.revision_id
            && self.generation == publication.generation
            && self.bundle_digest == publication.bundle_digest
    }
}

/// What one runtime file turned out to be on disk, as the writer read it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReadback {
    pub path: String,
    pub digest: Sha256Digest,
    pub bytes: u64,
    pub executable: bool,
    pub content_check: Check,
    pub mode_check: Check,
}

/// Body of `POST /api/v1/consumer/receipts`: what was read back from disk for
/// one installed publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptRequest {
    pub installation_id: ConsumerInstallationId,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: Sha256Digest,
    pub host: String,
    pub observed_at: DateTime<Utc>,
    pub files: Vec<CanonicalFile>,
    pub runtime_files: Vec<RuntimeReadback>,
}

impl ReceiptRequest {
    #[must_use]
    pub fn fully_verified(&self) -> bool {
        let verified = |executable: bool, content: Check, mode: Check| {
            content == Check::Verified
                && (mode == Check::Verified || (!executable && mode == Check::Unavailable))
        };
        !self.runtime_files.is_empty()
            && self
                .runtime_files
                .iter()
                .all(|f| verified(f.executable, f.content_check, f.mode_check))
            && self
                .files
                .iter()
                .all(|f| verified(f.executable, f.content_check, f.mode_check))
    }
}

/// Why a readback does not make a receipt: the path that does not hold.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("runtime file {path} does not match the plan")]
pub struct ReadbackMismatch {
    pub path: String,
}

pub fn build_receipt(
    plan: &InstallationPlan,
    readback: &[RuntimeReadback],
    installation_id: ConsumerInstallationId,
    observed_at: DateTime<Utc>,
) -> Result<ReceiptRequest, ReadbackMismatch> {
    let mut runtime = Vec::with_capacity(plan.runtime_files.len());
    for file in &plan.runtime_files {
        let digest = sha256_hex(&file.bytes);
        let size = u64::try_from(file.bytes.len()).unwrap_or(u64::MAX);
        let found = readback
            .iter()
            .find(|entry| entry.path == file.path)
            .filter(|found| {
                found.content_check == Check::Verified
                    && found.mode_check != Check::Mismatch
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
        files.push(CanonicalFile {
            content_check: copy.content_check,
            mode_check: copy.mode_check,
            ..file.clone()
        });
    }
    files.sort_by(|a, b| {
        (a.revision_id.as_str(), a.path.as_str()).cmp(&(b.revision_id.as_str(), b.path.as_str()))
    });
    runtime.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ReceiptRequest {
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

/// How the instance acknowledged a receipt; `identical_retry` is the same
/// receipt again, answered with the first `receipt_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Acknowledgement {
    Accepted,
    IdenticalRetry,
    #[serde(other)]
    Other,
}

/// Answer of `POST /api/v1/consumer/receipts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptResponse {
    pub receipt_id: InstallationReceiptId,
    pub acknowledgement: Acknowledgement,
    pub acknowledged_at: DateTime<Utc>,
    pub fully_verified: bool,
}

/// Body of `POST /api/v1/consumer/session-bindings`: binds a session to an
/// acknowledged receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBindingRequest {
    pub receipt_id: InstallationReceiptId,
    pub host: String,
    pub session_id: NativeSessionId,
}

/// Answer of `POST /api/v1/consumer/session-bindings`; the same binding again
/// answers the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBinding {
    pub id: InstallationSessionBindingId,
    pub bound_at: DateTime<Utc>,
}
