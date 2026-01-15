#![allow(clippy::incompatible_msrv)]

pub mod api;
pub mod assets;
pub mod build;
pub mod content;
pub mod jobs;
pub mod prerender;
pub mod sitemap;
pub mod templates;

pub use assets::{copy_implementation_assets, organize_css_files, organize_js_files};
pub use build::{BuildError, BuildMode, BuildOrchestrator};
pub use content::{extract_frontmatter, render_markdown};
pub use prerender::{prerender_content, prerender_homepage};
pub use sitemap::{build_sitemap_index, build_sitemap_xml, generate_sitemap, SitemapUrl};
pub use systemprompt_models::{ContentConfigRaw, ContentSourceConfigRaw, SitemapConfig};
pub use systemprompt_templates::TemplateRegistry;
pub use templates::{generate_footer_html, load_web_config, prepare_template_data};

pub use jobs::PublishContentJob;
