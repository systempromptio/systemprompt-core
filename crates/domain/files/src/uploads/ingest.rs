//! [`UploadService::ingest`]: bytes that reach the document store other than
//! through an upload (a bridge drive commit, a REST whole-file write) are
//! stored as an upload first, so they pass the same quota, sniff, zip guard
//! and image strip, and must have passed the check their format needs. The
//! bytes handed back are the stored blob's, which an image strip may have
//! rewritten.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::documents::{DocumentFormat, content_hash};

use super::error::{RefusalReason, UploadError, UploadResult};
use super::models::{BlobOrigin, NewUpload};
use super::service::UploadService;

const OPAQUE_MIME: &str = "application/octet-stream";

/// Bytes that passed the upload checks, and the MIME type recorded for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingested {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// The type a `file` document is declared as from its extension, so the
/// kinds that have a check (PDF, images, SVG) go through it.
fn file_mime(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => OPAQUE_MIME,
    }
}

impl UploadService {
    pub async fn ingest(
        &self,
        owner: &UserId,
        name: &str,
        format: DocumentFormat,
        bytes: Vec<u8>,
        trace: &TraceId,
    ) -> UploadResult<Ingested> {
        let file_name = name
            .rsplit(['/', '\\'])
            .next()
            .filter(|leaf| !leaf.trim().is_empty())
            .unwrap_or("upload")
            .to_owned();
        let mime = if format.is_file() {
            file_mime(&file_name)
        } else {
            format.mime_type()
        };
        let request = NewUpload {
            mime: mime.to_owned(),
            size: bytes.len() as u64,
            sha256: content_hash(&bytes),
            name: file_name,
        };
        let done = self.put_whole(owner, request, bytes, trace).await?;
        let origin = BlobOrigin {
            mime: done.detected_mime.clone(),
            checked_kind: Some(done.checked_kind.clone()),
        };
        if !origin.admits(format) {
            return Err(UploadError::refused(
                RefusalReason::UncheckedFormat,
                format!(
                    "the bytes did not pass the content check a {format} document needs (they \
                     passed {})",
                    done.checked_kind
                ),
            ));
        }
        let bytes = self
            .blobs
            .read_all_primary(&done.blob, self.max_upload_bytes)
            .await?;
        Ok(Ingested {
            bytes,
            mime: done.detected_mime,
        })
    }
}
