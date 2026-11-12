//! PDF export of a revision, rendered on the blocking pool: a format with a
//! rich text model renders from it with the images the revision holds, and any
//! other format from its codec's projection, under the
//! `files.documents.limits.pdfPages` cap. It follows at most
//! `EXPORT_IMAGE_LIMIT` image references and embeds at most
//! `maxUploadBytes` of them in all: an image is read only while it fits what
//! is left of that budget, and the rest render as missing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::CodecError;

use super::DocumentService;
use super::markdown_export::{EXPORT_IMAGE_LIMIT, has_rich_model, held_image_refs};
use super::pdf::RenderedPdf;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::model::formats::render_rich_text;
use crate::documents::pdf::{PdfError, RenderRequest, pdf_file_name, render, render_rich};

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
        let format = document.format;
        let model = if has_rich_model(format) {
            let held = bytes.clone();
            blocking(move || Ok(render_rich_text(format, &held).ok())).await?
        } else {
            None
        };
        let mut media = BTreeMap::new();
        let mut budget = self.config.max_upload_bytes;
        let refs = model.as_ref().map(held_image_refs).unwrap_or_default();
        for reference in refs.into_iter().take(EXPORT_IMAGE_LIMIT) {
            if let Some(image) = self
                .revision_image(&document, &bytes, &reference, budget)
                .await
            {
                budget = budget.saturating_sub(image.len() as u64);
                media.insert(reference, image);
            }
        }
        let rendered = blocking(move || {
            let request = RenderRequest {
                name: &name,
                revision: &revision_id,
                created_at,
                max_pages,
            };
            let outcome = match &model {
                Some(model) => render_rich(model, &media, &request),
                None => render(&codec.project(&bytes)?, &request),
            };
            outcome.map_err(|error| match error {
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
