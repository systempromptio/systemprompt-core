mod generator;
mod xml;

pub use generator::generate_sitemap;
pub use xml::{build_sitemap_index, build_sitemap_xml, SitemapUrl};
