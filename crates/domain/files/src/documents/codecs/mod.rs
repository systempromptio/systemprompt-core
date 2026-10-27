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
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod anchors;
mod html;
mod lines;
mod markdown;
mod placeholders;
mod registry;
mod text;

pub use html::HtmlCodec;
pub use markdown::MarkdownCodec;
pub use registry::CodecRegistry;
pub use text::TextCodec;

#[cfg(feature = "office")]
pub mod docx;

#[cfg(feature = "office")]
pub mod xlsx;
