//! Markdown rendering and frontmatter extraction used by the prerender
//! pipeline.

mod markdown;

pub use markdown::{extract_frontmatter, render_markdown};
