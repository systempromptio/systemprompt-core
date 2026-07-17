//! Prerender pipeline: turns the content stored in the database into static
//! HTML files in the build output directory.
//!
//! Public surface:
//!
//! - [`prerender_content`] / [`prerender_pages`] — top-level entry points
//! - [`PagePrerenderResult`] — outcome of a single page prerenderer
//!
//! Internal helpers are organised by responsibility: `context` (config + DI),
//! `fetch` (database access), `content` (per-source orchestration), `render`
//! (per-item HTML rendering), `list` (parent / index pages), `toc`
//! (table-of-contents generation), and `utils` (JSON merge and component
//! rendering).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod context;
mod engine;
mod fetch;
mod list;
mod render;
mod toc;
mod utils;

pub use engine::{PagePrerenderResult, prerender_content, prerender_pages};
pub use toc::{TocResult, generate_toc};
pub use utils::merge_json_data;
