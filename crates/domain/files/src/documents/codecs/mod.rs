//! Core document codecs and the registry that selects one per format.
//!
//! Four codecs ship with core, plus the office codecs under the `office`
//! feature; [`core_codecs`] lists them and the files member contributes them
//! to [`DocumentsSlot`](crate::contract::DocumentsSlot):
//!
//! - [`FileCodec`] — any other upload, stored as bytes and never edited.
//! - [`TextCodec`] — plain text, projected as `line[N]` nodes.
//! - [`MarkdownCodec`] — the same line nodes plus a read-only block outline
//!   (`h[N]`, `p[N]`, `li[N]`).
//! - [`HtmlCodec`] — block-level elements under `body`, addressed by tag and
//!   document order (`body/p[2]`).
//!
//! Every core codec reports the default priority (100). An extension that
//! contributes a codec for the same format with a higher priority replaces
//! the core codec in [`CodecRegistry`]; among equal priorities the codec
//! contributed first wins.
//!
//! Anchored ops in one batch address the projection they were read from.
//! A whole-document op (`str_replace`, `set_text`, `set_html`) rewrites that
//! projection, so an anchored op after it in the same batch is rejected as
//! malformed; submit it in a new batch against a fresh projection.
//!
//! Markdown content (`as: markdown` on `insert_after`, and
//! `append_markdown`) is parsed once into the format-neutral blocks of
//! `markdown_lowering`; each codec writes those blocks as its own markup.
//! Codecs apply resolved batches only: quote targets and Markdown
//! `replace_text` ops are resolved by
//! [`resolve_targets`](systemprompt_provider_contracts::documents::resolve_targets)
//! first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anchors;
pub mod file;
mod html;
mod line_nodes;
mod lines;
mod markdown;
mod markdown_lowering;
mod pending;
mod placeholders;
mod registry;
mod text;

#[cfg(feature = "office")]
mod fragments;
#[cfg(feature = "office")]
mod xml_tree;
#[cfg(feature = "office")]
mod zip_package;

pub use file::FileCodec;
pub use html::HtmlCodec;
pub use markdown::MarkdownCodec;
pub use registry::CodecRegistry;
pub use text::TextCodec;

#[cfg(feature = "office")]
pub use zip_package::{PackageCaps, inspect_package, media_part, media_part_by_stem};

#[cfg(feature = "office")]
pub mod docx;

#[cfg(feature = "office")]
pub mod xlsx;

#[cfg(feature = "office")]
pub mod pptx;

#[cfg(feature = "office")]
pub mod odf;

use std::sync::Arc;

use systemprompt_provider_contracts::documents::DocumentCodec;

#[must_use]
pub fn core_codecs() -> Vec<Arc<dyn DocumentCodec>> {
    let mut codecs: Vec<Arc<dyn DocumentCodec>> = vec![
        Arc::new(FileCodec),
        Arc::new(TextCodec),
        Arc::new(MarkdownCodec),
        Arc::new(HtmlCodec),
    ];
    #[cfg(feature = "office")]
    {
        let office: [Arc<dyn DocumentCodec>; 6] = [
            Arc::new(docx::DocxCodec),
            Arc::new(xlsx::XlsxCodec),
            Arc::new(pptx::PptxCodec),
            Arc::new(odf::OdtCodec),
            Arc::new(odf::OdsCodec),
            Arc::new(odf::OdpCodec),
        ];
        codecs.extend(office);
    }
    codecs
}
