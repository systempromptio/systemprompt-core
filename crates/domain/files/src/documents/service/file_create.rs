//! Creating a document from one uploaded file: the bytes are stored as an
//! upload (same content checks, quota and recorded MIME type as the resumable
//! API), read back from that blob and imported. A failed import retracts the
//! upload, so no blob is left behind for a caller who never learns its
//! `blob_ref`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentFormat, content_hash};

use super::ingest::refused;
use super::support::format_of_name;
use super::{Author, DocumentService, ImportRequest};
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::uploads::{Allowance, NewUpload};

/// A single file to turn into a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCreateRequest {
    pub name: String,
    pub folder: Option<DocumentFolderId>,
    pub format: Option<DocumentFormat>,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub author: Author,
}

impl DocumentService {
    pub async fn create_from_file(
        &self,
        who: &AuthenticatedUser,
        request: FileCreateRequest,
        trace: &TraceId,
    ) -> Result<(Document, DocumentRevision), DocumentsError> {
        let uploads = self.uploads.get().ok_or(DocumentsError::ContentRefused {
            code: "uploads_unavailable",
            detail: "no upload service is attached to store this file".to_owned(),
        })?;
        let FileCreateRequest {
            name,
            folder,
            format,
            mime,
            bytes,
            author,
        } = request;
        let format = format
            .or_else(|| format_of_name(&name))
            .unwrap_or(DocumentFormat::File);
        let upload = NewUpload {
            name: name.clone(),
            mime,
            size: bytes.len() as u64,
            sha256: content_hash(&bytes),
        };
        let done = uploads
            .put_whole(&who.id, upload, bytes, trace, Allowance::OneShot)
            .await
            .map_err(refused)?;
        let imported = async {
            let (bytes, origin) = uploads
                .read_owned_blob(&who.id, &done.blob, format, self.config.max_upload_bytes)
                .await
                .map_err(refused)?;
            let import = ImportRequest {
                folder,
                name,
                format: Some(format),
                mime_type: Some(origin.mime),
                bytes,
                author,
            };
            self.import(who, import, trace).await
        }
        .await;
        if imported.is_err()
            && let Err(error) = uploads.retract(&who.id, &done.session.id).await
        {
            tracing::warn!(
                upload_id = %done.session.id,
                error = %error,
                "Failed to retract the upload of a file that did not import"
            );
        }
        imported
    }
}
