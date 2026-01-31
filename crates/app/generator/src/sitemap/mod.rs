mod default_provider;
mod generator;
mod xml;

pub use default_provider::DefaultSitemapProvider;
pub use generator::generate_sitemap;
pub use xml::{build_sitemap_index, build_sitemap_xml, escape_xml, SitemapUrl};
