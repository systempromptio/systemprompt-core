mod default_provider;
mod generator;
mod xml;

pub use default_provider::DefaultRssFeedProvider;
pub use generator::{generate_feed, generate_feed_with_providers, GeneratedFeed};
pub use xml::{build_rss_xml, RssChannel, RssItem};
