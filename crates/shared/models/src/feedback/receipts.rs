//! Skill feedback contracts shared across ingestion, marketplace, evaluators and clients.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, ManagedResourceId,
    NativeSessionId, PublicationId, ResourceRevisionId, UserId,
};

use super::{ContentDigest, EvaluatorClient, FeedbackContractError, validate_relative_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedConsumerDevice {
    pub consumer_id: UserId,
    pub device_id: DeviceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadbackStatus {
    Verified,
    Mismatch,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        Ok(())
    }

    pub fn fully_verified(&self) -> bool {
        self.validate().is_ok() && self.files.iter().all(|file| {
            file.content_check == ReadbackStatus::Verified
                && file.mode_check == ReadbackStatus::Verified
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBindingRequest {
    pub receipt_id: InstallationReceiptId,
    pub host: EvaluatorClient,
    pub session_id: NativeSessionId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptAcknowledgement {
    Accepted,
    IdenticalRetry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerReceiptResponse {
    pub receipt_id: InstallationReceiptId,
    pub acknowledgement: ReceiptAcknowledgement,
    pub acknowledged_at: DateTime<Utc>,
    pub fully_verified: bool,
}
