//! Consumer receipts: the authenticated device, byte-exact file readbacks and
//! the installation plan a consumer acknowledges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, ManagedResourceId, NativeSessionId,
    PublicationId, ResourceRevisionId, UserId,
};

use super::{ContentDigest, EvaluatorClient, FeedbackContractError, validate_relative_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedConsumerDevice {
    pub consumer_id: UserId,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadbackStatus {
    Verified,
    Mismatch,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileReadback {
    pub revision_id: ResourceRevisionId,
    pub path: String,
    pub digest: ContentDigest,
    pub bytes: u64,
    pub executable: bool,
    pub content_check: ReadbackStatus,
    pub mode_check: ReadbackStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeFileReadback {
    pub path: String,
    pub digest: ContentDigest,
    pub bytes: u64,
    pub executable: bool,
    pub content_check: ReadbackStatus,
    pub mode_check: ReadbackStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InstallationPlanFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConsumerInstallationPlan {
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: ContentDigest,
    pub host: EvaluatorClient,
    pub canonical_files: Vec<FileReadback>,
    pub runtime_files: Vec<InstallationPlanFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerReceiptRequest {
    pub installation_id: ConsumerInstallationId,
    pub publication_id: PublicationId,
    pub resource_id: ManagedResourceId,
    pub revision_id: ResourceRevisionId,
    pub generation: i64,
    pub bundle_digest: ContentDigest,
    pub host: EvaluatorClient,
    pub observed_at: DateTime<Utc>,
    pub files: Vec<FileReadback>,
    #[serde(default)]
    pub runtime_files: Vec<RuntimeFileReadback>,
}

impl ConsumerReceiptRequest {
    pub fn validate(&self) -> Result<(), FeedbackContractError> {
        if self.generation < 1 || self.files.is_empty() || self.files.len() > 4096 {
            return Err(FeedbackContractError::Bounds);
        }
        let mut paths = std::collections::BTreeSet::new();
        for file in &self.files {
            validate_relative_path(&file.path)?;
            if !paths.insert((&file.revision_id, &file.path)) {
                return Err(FeedbackContractError::IncompleteManifest);
            }
        }
        let mut runtime_paths = std::collections::BTreeSet::new();
        if self.runtime_files.len() > 8192 {
            return Err(FeedbackContractError::Bounds);
        }
        for file in &self.runtime_files {
            validate_relative_path(&file.path)?;
            if !runtime_paths.insert(&file.path) {
                return Err(FeedbackContractError::IncompleteManifest);
            }
        }
        Ok(())
    }

    pub fn fully_verified(&self) -> bool {
        self.validate().is_ok()
            && !self.runtime_files.is_empty()
            && self
                .runtime_files
                .iter()
                .all(|file| file_verified(file.executable, file.content_check, file.mode_check))
            && self
                .files
                .iter()
                .all(|file| file_verified(file.executable, file.content_check, file.mode_check))
    }
}

// Why: a file that is not meant to be executable has no mode to satisfy, so a
// host without POSIX mode bits (Windows) reporting the mode as unavailable
// leaves nothing unchecked; the content digest is the whole check. An
// executable still needs its bit confirmed.
fn file_verified(executable: bool, content: ReadbackStatus, mode: ReadbackStatus) -> bool {
    content == ReadbackStatus::Verified
        && (mode == ReadbackStatus::Verified
            || (!executable && mode == ReadbackStatus::Unavailable))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionBindingRequest {
    pub receipt_id: InstallationReceiptId,
    pub host: EvaluatorClient,
    pub session_id: NativeSessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptAcknowledgement {
    Accepted,
    IdenticalRetry,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConsumerReceiptResponse {
    pub receipt_id: InstallationReceiptId,
    pub acknowledgement: ReceiptAcknowledgement,
    pub acknowledged_at: DateTime<Utc>,
    pub fully_verified: bool,
}
