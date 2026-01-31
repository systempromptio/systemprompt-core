#![allow(clippy::incompatible_msrv)]

pub mod api;
pub mod assets;
pub mod build;
pub mod content;
pub mod error;
pub mod jobs;
pub mod prerender;
pub mod rss;
pub mod sitemap;
pub mod templates;

pub use assets::organize_dist_assets;
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use error::PublishError;
pub use prerender::{prerender_content, prerender_pages, PagePrerenderResult};
pub use rss::{
    build_rss_xml, generate_feed, generate_feed_with_providers, DefaultRssFeedProvider,
    GeneratedFeed, RssChannel, RssItem,
};
pub use sitemap::{
    build_sitemap_index, build_sitemap_xml, escape_xml, generate_sitemap, DefaultSitemapProvider,
    SitemapUrl,
};
pub use systemprompt_models::{ContentConfigRaw, ContentSourceConfigRaw, SitemapConfig};
pub use systemprompt_templates::TemplateRegistry;
pub use templates::load_web_config;

pub use jobs::{execute_copy_extension_assets, execute_publish_content, ContentPublishJob};
