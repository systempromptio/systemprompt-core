//! The flat block outline every codec projection lowers to.
//!
//! Its Markdown rendering is the export a format without a rich text model is
//! written as. The `pdf` feature lays the same blocks out as pages; without it
//! the styling the blocks carry is never read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg_attr(
    not(feature = "pdf"),
    expect(dead_code, reason = "block styling is read only by the PDF renderer")
)]
pub(crate) mod blocks;
mod markdown;
#[cfg_attr(
    not(feature = "pdf"),
    expect(dead_code, reason = "run styling is read only by the PDF renderer")
)]
pub(crate) mod runs;
mod sheet_rows;

pub use markdown::{MARKDOWN_MIME_TYPE, markdown_file_name, outline_markdown};
