mod generator;
mod xml;

pub use generator::generate_feed;
pub use xml::{build_rss_xml, RssChannel, RssItem};
