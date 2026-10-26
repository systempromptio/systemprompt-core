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
//! (table-of-contents generation), `json_merge` (provider JSON deep merge),
//! and `components` (template component rendering).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod components;
mod content;
mod context;
mod engine;
mod fetch;
mod json_merge;
mod list;
mod render;
mod toc;

pub use engine::{PagePrerenderResult, PrerenderEnv, prerender_content, prerender_pages};
pub use json_merge::merge_json_data;
pub use toc::{TocResult, generate_toc};
