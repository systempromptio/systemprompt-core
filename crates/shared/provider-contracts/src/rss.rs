//! [`RssFeedProvider`] contract for emitting RSS feed metadata + items.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::SourceId;

use crate::error::ProviderResult;

/// Per-call context for RSS feed generation.
#[derive(Debug)]
pub struct RssFeedContext<'a> {
    /// Site base URL used to absolutize relative links.
    pub base_url: &'a str,
    /// Logical content source name driving this feed.
    pub source_name: &'a str,
}

/// Channel-level metadata for a generated RSS feed.
#[derive(Debug, Clone)]
pub struct RssFeedMetadata {
    /// Feed title.
    pub title: String,
    /// Canonical link for the feed's HTML counterpart.
    pub link: String,
    /// Feed description.
    pub description: String,
    /// Optional ISO-639 language tag.
    pub language: Option<String>,
}

/// A single item in a generated RSS feed.
#[derive(Debug, Clone)]
pub struct RssFeedItem {
    /// Item title.
    pub title: String,
    /// Canonical permalink to the item.
    pub link: String,
    /// Item summary or full text.
    pub description: String,
    /// Publication timestamp.
    pub pub_date: DateTime<Utc>,
    /// Stable globally-unique id for the item.
    pub guid: String,
    /// Optional author byline.
    pub author: Option<String>,
}

/// Static description of a feed this provider emits.
#[derive(Debug, Clone)]
pub struct RssFeedSpec {
    /// Content source feeding this RSS file.
    pub source_id: SourceId,
    /// Maximum number of items to emit.
    pub max_items: i64,
    /// Filename to write the feed to (relative to the dist root).
    pub output_filename: String,
}

/// Source-of-truth contract for RSS feed generation.
///
/// Marked `#[async_trait]` because it is consumed via `dyn RssFeedProvider`.
#[async_trait]
pub trait RssFeedProvider: Send + Sync {
    /// Stable identifier for this provider.
    fn provider_id(&self) -> &'static str;

    /// Static list of feeds this provider emits.
    fn feed_specs(&self) -> Vec<RssFeedSpec>;

    /// Channel-level metadata for the feed currently being generated.
    async fn feed_metadata(&self, ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata>;

    /// Fetch up to `limit` items, newest first.
    async fn fetch_items(
        &self,
        ctx: &RssFeedContext<'_>,
        limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>>;

    /// Provider priority; higher values override lower ones.
    fn priority(&self) -> u32 {
        100
    }
}
