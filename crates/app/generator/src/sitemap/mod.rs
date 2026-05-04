//! Sitemap generation: pure-function XML serialisers, the default provider
//! that drives them from `content.yaml`, and the top-level `generate_sitemap`
//! entry point.

mod default_provider;
mod generator;
mod xml;

pub use default_provider::DefaultSitemapProvider;
pub use generator::generate_sitemap;
pub use xml::{SitemapUrl, build_sitemap_index, build_sitemap_xml, escape_xml};
