//! A pure-Rust PDF rendering of a document revision, built from the codec
//! projection rather than from the stored bytes or the HTML preview.
//!
//! Every format lowers to the same blocks: headings by level, paragraphs,
//! list items with markers and nesting, tables as ruled grids with wrapped
//! cell text, one section per sheet with a grid of its used range, and one
//! page per slide with its title and body text. Plain-text lines and
//! `line[N]` nodes are set in Courier; everything else in Helvetica. Images
//! are omitted, never fetched. Pages are A4 with a document name and page
//! number footer, the PDF title is the document name, the subject names the
//! revision, and the creation date is the revision's, so the same revision
//! renders to the same bytes.
//!
//! Text is shown in `WinAnsiEncoding` with the standard fonts, so a
//! character outside that repertoire is drawn as `?`.
//!
//! The page count is capped by [`RenderRequest::max_pages`]; layout stops
//! at the cap and reports [`PdfError::PageCap`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blocks;
mod fonts;
mod layout;
mod table;
mod wrap;
mod writer;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_provider_contracts::documents::Projection;

pub use layout::PageCapExceeded;

pub const PDF_MIME_TYPE: &str = "application/pdf";

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error(transparent)]
    PageCap(#[from] PageCapExceeded),
    #[error("the PDF could not be written")]
    Write(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy)]
pub struct RenderRequest<'a> {
    pub name: &'a str,
    pub revision: &'a DocumentRevisionId,
    pub created_at: DateTime<Utc>,
    pub max_pages: usize,
}

pub fn render(projection: &Projection, request: &RenderRequest<'_>) -> Result<Vec<u8>, PdfError> {
    let blocks = blocks::blocks_of(projection);
    let mut pages = layout::Layout::paginate(&blocks, request.max_pages)?;
    layout::footer(&mut pages, request.name);
    let subject = format!("revision {}", request.revision);
    Ok(writer::write(
        &pages,
        writer::Metadata {
            title: request.name,
            subject: &subject,
            created_at: request.created_at,
        },
    )?)
}

#[must_use]
pub fn pdf_file_name(name: &str) -> String {
    let stem = name
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty() && !ext.contains(' '))
        .map_or(name, |(stem, _)| stem);
    format!("{stem}.pdf")
}
