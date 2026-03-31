#![allow(clippy::incompatible_msrv)]

pub(crate) mod api;
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
pub use error::PublishError;
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
