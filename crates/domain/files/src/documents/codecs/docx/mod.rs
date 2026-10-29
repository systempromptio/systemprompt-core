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
//! The `RichText` model of a package ([`import_rich`],
//! [`materialise_rich_with`]) reads and writes much more: paragraph and run
//! formatting, numbering, editable tables, drawings with their picture
//! parts, sections with header and footer parts, fields, styles and theme.
//!
//! The parts are addressed by the `w:` prefix Word always writes for the
//! main `WordprocessingML` namespace. Text inside text boxes, headers,
//! footers, footnotes and comments is not projected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod blank;
mod comments;
mod fragment;
mod import;
mod inline;
mod insert;
mod instantiate;
mod links;
mod markdown;
mod markup;
mod numbering;
mod package;
mod parts;
mod preview;
mod projection;
mod read;
mod read_drawing;
mod read_numbering;
mod read_runs;
mod read_section;
mod read_styles;
mod read_table;
mod rich;
mod scan;
mod sect_splice;
mod sections;
mod style_merge;
mod styles;
mod write;
mod write_drawing;
mod write_runs;
mod write_styles;
mod write_table;

pub use comments::{NativeComment, with_comments};
pub use read_drawing::{MAX_PART_BYTES, held_image, saved_upload_hashes};
pub use rich::{import_rich, materialise_rich, materialise_rich_with};

use std::sync::Arc;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, NodeFragment, NodeTarget, Projection,
    TemplateParams, content_hash, fragment_insert_op,
};

use crate::documents::codecs::fragments::leaf_hashes;
use systemprompt_provider_contracts::register_document_codec;

pub use blank::blank_docx;

/// The `.docx` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct DocxCodec;

impl DocumentCodec for DocxCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["heading", "paragraph", "list_item", "pending"]
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

    fn style_hashes(&self, bytes: &[u8]) -> Result<Vec<(DocumentNodeId, String)>, CodecError> {
        Ok(leaf_hashes(
            &self.project(bytes)?,
            &fragment::styled(bytes)?,
        ))
    }

    fn copy_node(
        &self,
        bytes: &[u8],
        base: &Projection,
        path: &DocumentNodeId,
    ) -> Result<NodeFragment, CodecError> {
        fragment::copy(bytes, base, path)
    }

    fn insert_fragment(
        &self,
        bytes: &[u8],
        base: &Projection,
        after: &NodeTarget,
        fragment: &NodeFragment,
    ) -> Result<Vec<u8>, CodecError> {
        if fragment.native.is_none() {
            return self.apply(bytes, base, &[fragment_insert_op(after, fragment)]);
        }
        fragment::insert(bytes, base, after, fragment)
    }
}

fn docx_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(DocxCodec)
}

register_document_codec!(docx_codec);
