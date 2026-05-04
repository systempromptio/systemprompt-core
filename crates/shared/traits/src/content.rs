//! Content provider traits for blog posts, docs, and other published items.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContentId, SourceId};

/// Lightweight summary returned by listing endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    /// Stable content identifier.
    pub id: ContentId,
    /// URL slug.
    pub slug: String,
    /// Display title.
    pub title: String,
    /// One-line description.
    pub description: String,
    /// Publication timestamp.
    pub published_at: DateTime<Utc>,
    /// Content kind tag (`blog`, `doc`, ...).
    pub kind: String,
    /// Originating source.
    pub source_id: SourceId,
}

/// Full content payload returned by detail endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    /// Stable content identifier.
    pub id: ContentId,
    /// URL slug.
    pub slug: String,
    /// Display title.
    pub title: String,
    /// One-line description.
    pub description: String,
    /// Rendered body (markdown / HTML).
    pub body: String,
    /// Author display name.
    pub author: String,
    /// Publication timestamp.
    pub published_at: DateTime<Utc>,
    /// Comma-separated keywords for SEO.
    pub keywords: String,
    /// Content kind tag.
    pub kind: String,
    /// Optional hero image URL.
    pub image: Option<String>,
    /// Originating source.
    pub source_id: SourceId,
    /// Optional category id.
    pub category_id: Option<String>,
}

/// Filter criteria accepted by [`ContentProvider::list_content`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFilter {
    /// Restrict to a specific source.
    pub source_id: Option<String>,
    /// Restrict to a specific category.
    pub category_id: Option<String>,
    /// Restrict to a specific kind (`blog`, `doc`, ...).
    pub kind: Option<String>,
    /// Free-text search term.
    pub query: Option<String>,
    /// Max rows to return.
    pub limit: Option<i64>,
    /// Pagination offset.
    pub offset: Option<i64>,
}

/// Read-only content store abstraction.
///
/// `#[async_trait]` is required because the trait is consumed as
/// `dyn ContentProvider<...>`. Implementors choose their own error type
/// via the `Error` associated type so the trait stays decoupled from any
/// specific persistence layer.
#[async_trait]
pub trait ContentProvider: Send + Sync {
    /// Implementation-specific error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Look up a content item by id.
    async fn get_content(&self, id: &str) -> Result<Option<ContentItem>, Self::Error>;

    /// Look up a content item by slug.
    async fn get_content_by_slug(&self, slug: &str) -> Result<Option<ContentItem>, Self::Error>;

    /// Look up a content item by source plus slug.
    async fn get_content_by_source_and_slug(
        &self,
        source_id: &str,
        slug: &str,
    ) -> Result<Option<ContentItem>, Self::Error>;

    /// List content matching `filter`.
    async fn list_content(&self, filter: ContentFilter)
    -> Result<Vec<ContentSummary>, Self::Error>;

    /// Run a free-text search over content.
    async fn search(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<ContentSummary>, Self::Error>;
}
