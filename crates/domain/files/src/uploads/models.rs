//! Upload session and part rows, and the request that opens a session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, TraceId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    Open,
    Completed,
    Refused,
    Aborted,
    Expired,
}

impl UploadStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Completed => "completed",
            Self::Refused => "refused",
            Self::Aborted => "aborted",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not an upload status")]
pub struct UnknownUploadStatus(pub String);

impl std::str::FromStr for UploadStatus {
    type Err = UnknownUploadStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "completed" => Ok(Self::Completed),
            "refused" => Ok(Self::Refused),
            "aborted" => Ok(Self::Aborted),
            "expired" => Ok(Self::Expired),
            other => Err(UnknownUploadStatus(other.to_owned())),
        }
    }
}

impl std::fmt::Display for UploadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSession {
    pub id: DocumentUploadId,
    pub owner_user_id: UserId,
    pub file_name: String,
    pub declared_mime: String,
    pub declared_size: u64,
    pub declared_sha256: ContentHash,
    pub part_size: u64,
    pub status: UploadStatus,
    pub blob_sha256: Option<ContentHash>,
    pub detected_mime: Option<String>,
    pub refusal: Option<String>,
    pub trace_id: TraceId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl UploadSession {
    #[must_use]
    pub fn part_count(&self) -> u32 {
        part_count(self.declared_size, self.part_size)
    }

    #[must_use]
    pub fn expected_part_size(&self, part: u32) -> u64 {
        let before = u64::from(part.saturating_sub(1)).saturating_mul(self.part_size);
        self.declared_size
            .saturating_sub(before)
            .min(self.part_size)
    }
}

#[must_use]
pub fn part_count(size: u64, part_size: u64) -> u32 {
    u32::try_from(size.div_ceil(part_size.max(1))).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UploadPart {
    pub part_number: u32,
    pub size_bytes: u64,
    pub sha256: ContentHash,
    #[serde(skip)]
    pub storage_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUpload {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadProgress {
    pub session: UploadSession,
    pub parts: Vec<UploadPart>,
}

impl UploadProgress {
    #[must_use]
    pub fn received_bytes(&self) -> u64 {
        self.parts.iter().map(|part| part.size_bytes).sum()
    }

    #[must_use]
    pub fn missing_parts(&self) -> Vec<u32> {
        (1..=self.session.part_count())
            .filter(|n| {
                !self.parts.iter().any(|part| {
                    part.part_number == *n && part.size_bytes == self.session.expected_part_size(*n)
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedUpload {
    pub session: UploadSession,
    pub blob: ContentHash,
    pub size_bytes: u64,
    pub detected_mime: String,
    pub deduplicated: bool,
}
