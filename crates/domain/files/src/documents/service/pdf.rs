//! PDF export of a revision: the stored bytes are projected by the
//! format's codec and rendered on the blocking pool, under the
//! `files.documents.limits.pdfPages` cap.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::CodecError;

use super::DocumentService;
use super::support::blocking;
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
}
