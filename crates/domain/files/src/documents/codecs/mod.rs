//! Core document codecs and the registry that selects one per format.
//!
//! Three codecs ship with core and register themselves at compile time with
//! `register_document_codec!`:
//!
//! - [`TextCodec`] — plain text, projected as `line[N]` nodes.
//! - [`MarkdownCodec`] — the same line nodes plus a read-only block outline
//!   (`h[N]`, `p[N]`, `li[N]`).
//! - [`HtmlCodec`] — block-level elements under `body`, addressed by tag and
//!   document order (`body/p[2]`).
//!
//! Every core codec reports the default priority (100). An extension that
//! registers a codec for the same format with a higher priority replaces the
//! core codec in [`CodecRegistry`]; among equal priorities the codec
//! registered first wins.
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
mod html;
mod lines;
mod markdown;
mod markdown_lowering;
mod placeholders;
mod registry;
mod text;

#[cfg(feature = "office")]
mod xml_tree;
#[cfg(feature = "office")]
mod zip_package;

pub use html::HtmlCodec;
pub use markdown::MarkdownCodec;
pub use registry::CodecRegistry;
pub use text::TextCodec;

#[cfg(feature = "office")]
pub mod docx;

#[cfg(feature = "office")]
pub mod xlsx;

#[cfg(feature = "office")]
pub mod pptx;

#[cfg(feature = "office")]
pub mod odf;
