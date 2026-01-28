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
pub use prerender::{prerender_content, prerender_homepage};
pub use rss::{build_rss_xml, generate_feed, RssChannel, RssItem};
pub use sitemap::{build_sitemap_index, build_sitemap_xml, generate_sitemap, SitemapUrl};
pub use systemprompt_models::{ContentConfigRaw, ContentSourceConfigRaw, SitemapConfig};
pub use systemprompt_templates::TemplateRegistry;
pub use templates::{generate_footer_html, load_web_config, prepare_template_data};

pub use jobs::{CopyExtensionAssetsJob, PublishContentJob};
