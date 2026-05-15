//! [`RssFeedProvider`] contract for emitting RSS feed metadata + items.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::SourceId;

use crate::error::ProviderResult;

#[derive(Debug)]
pub struct RssFeedContext<'a> {
    pub base_url: &'a str,
    pub source_name: &'a str,
}

#[derive(Debug, Clone)]
pub struct RssFeedMetadata {
    pub title: String,
    pub link: String,
    pub description: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RssFeedItem {
    pub title: String,
    pub link: String,
    pub description: String,
    pub pub_date: DateTime<Utc>,
    pub guid: String,
    pub author: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RssFeedSpec {
    pub source_id: SourceId,
    pub max_items: i64,
    pub output_filename: String,
}

// Why: provider is consumed as a trait object by the generator crate; an
// async fn in a bare trait is not dyn-compatible, so #[async_trait] is required.
#[async_trait]
pub trait RssFeedProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;

    fn feed_specs(&self) -> Vec<RssFeedSpec>;

    async fn feed_metadata(&self, ctx: &RssFeedContext<'_>) -> ProviderResult<RssFeedMetadata>;

    async fn fetch_items(
        &self,
        ctx: &RssFeedContext<'_>,
        limit: i64,
    ) -> ProviderResult<Vec<RssFeedItem>>;

    fn priority(&self) -> u32 {
        100
    }
}
