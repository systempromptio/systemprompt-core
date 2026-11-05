//! A pure-Rust PDF rendering of a document revision.
//!
//! Documents with a rich text model (docx, odt, Markdown, HTML) render from
//! that model through [`render_rich`], so bold, italic, underline, strike,
//! colour, highlight, links, quotes, code, styled tables and the images the
//! revision holds survive. Sheets and decks render from the codec projection
//! through [`render`]: one section per sheet with a grid of its used range, and
//! one page per slide with its title and body text. Images are never fetched.
//! Pages are A4 with a document name and page number footer, the PDF title is
//! the document name, the subject names the revision, and the creation date is
//! the revision's, so the same revision renders to the same bytes.
//!
//! Text is drawn in subset embedded `DejaVu` faces with a `ToUnicode` map,
//! covering Latin, Greek and Cyrillic. There is no shaping: right-to-left and
//! CJK text is out of scope, and a character outside the faces is drawn as
//! U+FFFD.
//!
//! The page count is capped by [`RenderRequest::max_pages`]; layout stops
//! at the cap and reports [`PdfError::PageCap`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod blocks;
mod font_embed;
mod fonts;
mod images;
mod layout;
mod layout_model;
mod markdown;
mod rich;
mod runs;
mod sheet_rows;
mod table;
mod wrap;
mod writer;

use std::collections::BTreeMap;
use std::io::Write;

use chrono::{DateTime, Utc};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use systemprompt_identifiers::DocumentRevisionId;
use systemprompt_provider_contracts::documents::Projection;

use crate::documents::model::RichText;

pub use layout_model::PageCapExceeded;
pub use markdown::{MARKDOWN_MIME_TYPE, markdown_file_name, outline_markdown};

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
    finish(&blocks::blocks_of(projection), &[], request)
}

pub fn render_rich(
    model: &RichText,
    media: &BTreeMap<String, Vec<u8>>,
    request: &RenderRequest<'_>,
) -> Result<Vec<u8>, PdfError> {
    let lowered = rich::lower(model, media);
    finish(&lowered.blocks, &lowered.pictures, request)
}

fn finish(
    blocks: &[blocks::Block],
    pictures: &[images::Picture],
    request: &RenderRequest<'_>,
) -> Result<Vec<u8>, PdfError> {
    let mut pages = layout::Layout::paginate(blocks, pictures, request.max_pages)?;
    layout::footer(&mut pages, request.name);
    let subject = format!("revision {}", request.revision);
    Ok(writer::write(
        &pages,
        pictures,
        writer::Metadata {
            title: request.name,
            subject: &subject,
            created_at: request.created_at,
        },
    )?)
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
}

#[must_use]
pub fn pdf_file_name(name: &str) -> String {
    let stem = name
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty() && !ext.contains(' '))
        .map_or(name, |(stem, _)| stem);
    format!("{stem}.pdf")
}
