mod markdown;
mod toc;

pub use markdown::{extract_frontmatter, render_markdown};
pub use toc::{TocResult, generate_toc};
