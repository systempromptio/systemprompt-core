//! PDF export of a revision: the stored bytes are projected by the
//! format's codec and rendered on the blocking pool, under the
//! `files.documents.limits.pdfPages` cap. A stored `file` revision whose
//! bytes are a PDF opens as a stream for inline preview, decided by its magic
//! bytes alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::StreamExt;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::CodecError;
use systemprompt_traits::{ByteRange, FileStorageError, StoredFileRead};

use super::support::blocking;
use super::{DocumentService, StreamRequest};
use crate::documents::blob_store::BlobStoreError;
use crate::documents::codecs::file::is_pdf;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentRevision;
use crate::documents::pdf::{PdfError, RenderRequest, pdf_file_name, render};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPdf {
    pub revision: DocumentRevision,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

impl DocumentService {
    pub async fn render_pdf(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<RenderedPdf, DocumentsError> {
        let (document, revision, bytes) = self.read(who, document, revision, trace).await?;
        if document.format.is_file() {
            return Err(DocumentsError::UnsupportedFormat {
                format: document.format,
            });
        }
        let codec = self.codec(document.format)?;
        let max_pages = usize::try_from(self.config.limits.pdf_pages).unwrap_or(usize::MAX);
        let name = document.name.clone();
        let revision_id = revision.id.clone();
        let created_at = revision.created_at;
        let rendered = blocking(move || {
            let projection = codec.project(&bytes)?;
            let request = RenderRequest {
                name: &name,
                revision: &revision_id,
                created_at,
                max_pages,
            };
            render(&projection, &request).map_err(|error| match error {
                PdfError::PageCap(cap) => DocumentsError::TooLarge {
                    what: "rendered PDF (pages)",
                    size: u64::try_from(cap.max_pages).unwrap_or(u64::MAX) + 1,
                    limit: u64::try_from(cap.max_pages).unwrap_or(u64::MAX),
                },
                PdfError::Write(source) => DocumentsError::Codec(CodecError::Io(source)),
            })
        })
        .await?;
        Ok(RenderedPdf {
            file_name: pdf_file_name(&document.name),
            revision,
            bytes: rendered,
        })
    }

    pub async fn open_pdf_stream(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<Option<(DocumentRevision, StoredFileRead)>, DocumentsError> {
        let head = StreamRequest {
            document,
            revision,
            range: Some(ByteRange::Bounded { start: 0, end: 7 }),
        };
        let (_, mut read) = match self.open_stream(who, head, trace).await {
            Err(DocumentsError::Blob(BlobStoreError::Storage(
                FileStorageError::RangeNotSatisfiable { .. },
            ))) => return Ok(None),
            other => other?,
        };
        let mut magic = Vec::new();
        while let Some(chunk) = read.body.next().await {
            magic.extend_from_slice(
                &chunk.map_err(|source| DocumentsError::Blob(BlobStoreError::Storage(source)))?,
            );
        }
        if !is_pdf(&magic) {
            return Ok(None);
        }
        let whole = StreamRequest {
            document,
            revision,
            range: None,
        };
        self.open_stream(who, whole, trace).await.map(Some)
    }
}
