//! Static-site generation, theme rendering, and asset bundling for the
//! systemprompt.io AI governance dashboard. Drives the Handlebars + Markdown
//! pipeline that turns content rows in the database into a fully static
//! `dist/` directory.
//!
//! # Public surface
//!
//! - [`prerender_content`] / [`prerender_pages`] — main entry points for
//!   rendering content sources and registered page-prerenderer extensions.
//! - [`BuildOrchestrator`] / [`BuildMode`] — drives the whole build (CSS
//!   organisation + validation) with progress reporting.
//! - [`generate_sitemap`], [`generate_feed`] — emit `sitemap.xml` and
//!   per-source RSS feeds, over the install-readable `ConfigView`, so an
//!   install's own job can call them.
//! - [`organize_dist_assets`] — post-build CSS/JS file reorganisation.
//! - [`write_atomic`] / [`copy_atomic`] — temp-file-and-rename publication used
//!   for every file written into `dist/`.
//! - [`PublishError`] / [`GeneratorResult`] — the typed error and `Result`
//!   alias returned by every public function in this crate.
//! - [`ContentPrerenderJob`], [`PagePrerenderJob`] — scheduled jobs contributed
//!   by [`GeneratorExtension`] (id `generator`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod assets;
pub mod atomic;
pub(crate) mod build;
pub(crate) mod content;
pub(crate) mod error;
pub mod extension;
pub(crate) mod jobs;
pub(crate) mod prerender;
pub(crate) mod rss;
pub(crate) mod sitemap;
pub(crate) mod templates;

pub use assets::organize_dist_assets;
pub use atomic::{copy_atomic, write_atomic};
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use error::{GeneratorResult, PublishError};
pub use extension::{GENERATOR_EXTENSION, GeneratorExtension};
pub use prerender::{
    PagePrerenderResult, PopularContent, PrerenderEnv, PrerenderSettings, TocResult,
    fetch_popular_ids, generate_toc, merge_json_data, prerender_content, prerender_pages,
};
pub use rss::{
    DefaultRssFeedProvider, GeneratedFeed, RssChannel, RssItem, build_rss_xml, generate_feed,
    generate_feed_with_providers,
};
pub use sitemap::{
    DefaultSitemapProvider, SitemapUrl, build_sitemap_index, build_sitemap_xml, escape_xml,
    generate_sitemap,
};
pub use systemprompt_models::{ContentConfigRaw, ContentSourceConfigRaw, SitemapConfig};
pub use systemprompt_templates::TemplateRegistry;
pub use templates::{get_templates_path, load_web_config};

pub use jobs::{
    ContentPrerenderJob, PagePrerenderJob, copy_asset, declared_assets,
    execute_copy_extension_assets,
};
