//! The `.docx` (`WordprocessingML`) document codec.
//!
//! [`DocxCodec`] reads and writes only `word/document.xml`, plus
//! `word/styles.xml` read-only when an inserted heading needs a style id.
//! An edit rewrites the package so that every other entry keeps its name,
//! position and compressed bytes, and the edited part keeps every byte
//! outside the paragraphs an op touched: unknown elements, namespace
//! declarations, comments, processing instructions and the XML declaration
//! pass through untouched.
//!
//! Projection walks the body (descending through block-level content
//! controls): a paragraph is `pid:<w14:paraId>` when Word stamped one, else
//! `body/p[N]`; a table is `body/tbl[T]` with `tr[R]/tc[C]` children whose
//! paragraphs use the same rule under the cell's path. A paragraph whose
//! style is `Heading1`–`Heading9` or `Title` projects as a heading, one with
//! numbering properties as a list item, and drawings as image children
//! carrying their relationship id.
//!
//! The parts are addressed by the `w:` prefix Word always writes for the
//! main `WordprocessingML` namespace. Text inside text boxes, headers,
//! footers, footnotes and comments is not projected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod blank;
mod inline;
mod instantiate;
mod markup;
mod parts;
mod preview;
mod projection;
mod scan;
mod styles;

use std::sync::Arc;

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams, content_hash,
};
use systemprompt_provider_contracts::register_document_codec;

pub use blank::blank_docx;

/// The `.docx` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocxCodec;

impl DocumentCodec for DocxCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["heading", "paragraph", "list_item"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Docx]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let xml = parts::read_document(bytes)?;
        let doc = scan::parse(&xml)?;
        Ok(projection::project(bytes, &doc))
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        if base.content_sha256 != content_hash(bytes) {
            return Err(CodecError::Malformed {
                detail: "the base projection was not read from these document bytes",
            });
        }
        apply::apply(bytes, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let xml = parts::read_document(bytes)?;
        Ok(preview::render(&scan::parse(&xml)?.blocks))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let xml = parts::read_document(bytes)?;
        Ok(projection::blocks_text(&scan::parse(&xml)?.blocks))
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        instantiate::instantiate(template, params)
    }
}

fn docx_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(DocxCodec)
}

register_document_codec!(docx_codec);
