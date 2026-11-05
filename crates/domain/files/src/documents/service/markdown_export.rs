//! Markdown export of the head revision. A format with a rich text model is
//! degraded to what Markdown holds, and every other format is written from its
//! projection: a sheet as one table per sheet, a deck as a text outline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;

use super::DocumentService;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::model::formats::import_rich_text;
use crate::documents::model::markdown::{degrade, materialise};
use crate::documents::pdf::{markdown_file_name, outline_markdown};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedMarkdown {
    pub file_name: String,
    pub markdown: String,
}

const fn has_rich_model(format: DocumentFormat) -> bool {
    matches!(
        format,
        DocumentFormat::Docx
            | DocumentFormat::Odt
            | DocumentFormat::Markdown
            | DocumentFormat::Html
    )
}

impl DocumentService {
    pub async fn export_markdown(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<ExportedMarkdown, DocumentsError> {
        let (document, _, bytes) = self.read(who, document, None, trace).await?;
        if document.format.is_file() {
            return Err(DocumentsError::UnsupportedFormat {
                format: document.format,
            });
        }
        let codec = self.codec(document.format)?;
        let format = document.format;
        let markdown = blocking(move || {
            if has_rich_model(format)
                && let Ok(model) = import_rich_text(format, &bytes)
            {
                let (degraded, warnings) = degrade(&model);
                for warning in warnings {
                    tracing::info!(%format, warning = %warning, "markdown export left out content");
                }
                return Ok(materialise(&degraded)?);
            }
            Ok(outline_markdown(&codec.project(&bytes)?))
        })
        .await?;
        Ok(ExportedMarkdown {
            file_name: markdown_file_name(&document.name),
            markdown,
        })
    }
}
