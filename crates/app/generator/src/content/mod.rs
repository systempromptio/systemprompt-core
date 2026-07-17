//! Markdown rendering and frontmatter extraction used by the prerender
//! pipeline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod markdown;

pub use markdown::{extract_frontmatter, render_markdown};
