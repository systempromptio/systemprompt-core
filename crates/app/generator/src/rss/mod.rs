mod default_provider;
mod generator;
mod xml;

pub use default_provider::DefaultRssFeedProvider;
pub use generator::{GeneratedFeed, generate_feed, generate_feed_with_providers};
pub use xml::{RssChannel, RssItem, build_rss_xml};
