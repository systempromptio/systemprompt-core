//! Resumable uploads, the blob they complete into, and importing a blob as a
//! document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use systemprompt_identifiers::{DocumentFolderId, DocumentUploadId};

pub const INLINE_UPLOAD_LIMIT: usize = 8 * 1024 * 1024;

/// An upload in progress. Parts are numbered from 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveUpload {
    pub upload_id: DocumentUploadId,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
    pub part_size: u64,
    pub part_count: u32,
    pub status: String,
    #[serde(default)]
    pub received_bytes: u64,
    #[serde(default)]
    pub received_parts: Vec<u32>,
    #[serde(default)]
    pub missing_parts: Vec<u32>,
    #[serde(default)]
    pub blob_ref: Option<String>,
    pub expires_at: String,
}

impl DriveUpload {
    #[must_use]
    pub fn part_range(&self, number: u32) -> Option<std::ops::Range<u64>> {
        let index = u64::from(number.checked_sub(1)?);
        let start = index.checked_mul(self.part_size)?;
        if start >= self.size {
            return None;
        }
        Some(start..start.saturating_add(self.part_size).min(self.size))
    }
}

/// `PUT /api/v1/uploads/{id}/parts/{n}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveUploadPart {
    pub part_number: u32,
    pub size_bytes: u64,
    pub sha256: String,
}

/// `POST /api/v1/uploads/{id}/complete`: the blob, its ref the content's
/// SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveBlob {
    pub upload_id: DocumentUploadId,
    pub blob_ref: String,
    pub size_bytes: u64,
    pub mime: String,
    #[serde(default)]
    pub deduplicated: Option<bool>,
}

/// Body of `POST /api/v1/documents/import`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportBlobRequest {
    pub blob_ref: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<DocumentFolderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of `POST /api/v1/uploads`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenUploadRequest {
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
}
