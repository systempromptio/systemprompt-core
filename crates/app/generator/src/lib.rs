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
//!   per-source RSS feeds.
//! - [`organize_dist_assets`] — post-build CSS/JS file reorganisation.
//! - [`PublishError`] / [`GeneratorResult`] — the typed error and `Result`
//!   alias returned by every public function in this crate.
//! - [`ContentPrerenderJob`], [`PagePrerenderJob`] — scheduled jobs registered
//!   with the systemprompt scheduler via the `inventory` crate.
//!
//! # Feature flags
//!
//! | Feature             | Effect                                                                 |
//! | ------------------- | ---------------------------------------------------------------------- |
//! | `image-processing`  | Pulls in the `image` crate to enable WebP conversion in asset jobs.    |
//!
//! All features are off by default.

#![allow(clippy::incompatible_msrv)]

pub(crate) mod assets;
pub(crate) mod build;
pub(crate) mod content;
pub(crate) mod error;
pub(crate) mod jobs;
pub(crate) mod prerender;
pub(crate) mod rss;
pub(crate) mod sitemap;
pub(crate) mod templates;

pub use assets::organize_dist_assets;
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use error::{GeneratorResult, PublishError};
pub use prerender::{PagePrerenderResult, prerender_content, prerender_pages};
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
pub use templates::load_web_config;

pub use jobs::{ContentPrerenderJob, PagePrerenderJob, execute_copy_extension_assets};
