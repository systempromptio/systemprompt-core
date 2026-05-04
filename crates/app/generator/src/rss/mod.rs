//! RSS 2.0 feed generation: pure XML serialisers, the default provider that
//! drives them from `content.yaml` + the database, and the top-level
//! `generate_feed` entry point.

mod default_provider;
mod generator;
mod xml;

pub use default_provider::DefaultRssFeedProvider;
pub use generator::{GeneratedFeed, generate_feed, generate_feed_with_providers};
pub use xml::{RssChannel, RssItem, build_rss_xml};
