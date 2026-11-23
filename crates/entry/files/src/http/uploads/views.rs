//! The JSON bodies the upload routes answer with.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_files::uploads::{
    CompletedUpload, UploadPart, UploadProgress, UploadSession, UploadStatus,
};
use systemprompt_identifiers::{ContentHash, DocumentUploadId};

#[derive(Debug, Deserialize)]
pub(super) struct CreateBody {
    pub(super) name: String,
    pub(super) mime: String,
    pub(super) size: u64,
    pub(super) sha256: ContentHash,
}

#[derive(Debug, Serialize)]
pub(super) struct UploadJson {
    upload_id: DocumentUploadId,
    name: String,
    mime: String,
    size: u64,
    sha256: ContentHash,
    part_size: u64,
    part_count: u32,
    status: UploadStatus,
    received_bytes: u64,
    received_parts: Vec<u32>,
    parts: Vec<UploadPart>,
    missing_parts: Vec<u32>,
    blob_ref: Option<ContentHash>,
    detected_mime: Option<String>,
    refusal: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl From<UploadProgress> for UploadJson {
    fn from(progress: UploadProgress) -> Self {
        let received_bytes = progress.received_bytes();
        let received_parts = progress.parts.iter().map(|part| part.part_number).collect();
        let missing_parts = if progress.session.status == UploadStatus::Open {
            progress.missing_parts()
        } else {
            Vec::new()
        };
        let session = progress.session;
        Self {
            part_count: session.part_count(),
            upload_id: session.id,
            name: session.file_name,
            mime: session.declared_mime,
            size: session.declared_size,
            sha256: session.declared_sha256,
            part_size: session.part_size,
            status: session.status,
            received_bytes,
            received_parts,
            parts: progress.parts,
            missing_parts,
            blob_ref: session.blob_sha256,
            detected_mime: session.detected_mime,
            refusal: session.refusal,
            created_at: session.created_at,
            expires_at: session.expires_at,
        }
    }
}

impl From<UploadSession> for UploadJson {
    fn from(session: UploadSession) -> Self {
        Self::from(UploadProgress {
            session,
            parts: Vec::new(),
        })
    }
}

#[derive(Debug, Serialize)]
pub(super) struct CompletedJson {
    upload_id: DocumentUploadId,
    blob_ref: ContentHash,
    size_bytes: u64,
    mime: String,
    deduplicated: bool,
}

impl From<CompletedUpload> for CompletedJson {
    fn from(done: CompletedUpload) -> Self {
        Self {
            upload_id: done.session.id,
            blob_ref: done.blob,
            size_bytes: done.size_bytes,
            mime: done.detected_mime,
            deduplicated: done.deduplicated,
        }
    }
}
