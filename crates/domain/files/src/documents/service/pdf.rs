//! PDF delivery of a revision. With the `pdf` feature a revision renders
//! through `pdf_render`; without it a PDF export is refused as an unsupported
//! operation. A stored `file` revision whose bytes are a PDF opens as a stream
//! for inline preview, decided by its magic bytes alone, in every build.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use futures_util::StreamExt;
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_traits::{ByteRange, FileStorageError, StoredFileRead};

use super::{DocumentService, StreamRequest};
use crate::documents::blob_store::BlobStoreError;
use crate::documents::codecs::file::is_pdf;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentRevision;

#[derive(Debug, Clone, PartialEq)]
pub struct RenderedPdf {
    pub revision: DocumentRevision,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

impl DocumentService {
    #[cfg(not(feature = "pdf"))]
    pub async fn render_pdf(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        _revision: Option<&DocumentRevisionId>,
        trace: &TraceId,
    ) -> Result<RenderedPdf, DocumentsError> {
        let view = self.get(who, document, trace).await?;
        Err(DocumentsError::UnsupportedOp {
            op: "pdf_export",
            format: view.document.format,
            accepted_kinds: &[],
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
