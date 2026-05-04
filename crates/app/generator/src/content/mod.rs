//! Markdown rendering, frontmatter extraction, and table-of-contents
//! generation used by the prerender pipeline.

mod markdown;
mod toc;

pub use markdown::{extract_frontmatter, render_markdown};
pub use toc::generate_toc;
