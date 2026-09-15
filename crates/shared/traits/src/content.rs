//! Content provider traits for blog posts, docs, and other published items.
//!
//! [`ContentProvider`] carries an associated error type, so it is only ever
//! dispatched statically and uses native `async fn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

/// Authoritative content counts used by behavioral classification; `dyn`
/// dispatched, hence `#[async_trait]`.
#[async_trait]
pub trait ContentCatalogStats: Send + Sync + std::fmt::Debug {
    async fn count_public_pages(&self) -> Result<i64, crate::RepositoryError>;
}

pub type DynContentCatalogStats = std::sync::Arc<dyn ContentCatalogStats>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummary {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub published_at: DateTime<Utc>,
    pub kind: String,
    pub source_id: SourceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    pub keywords: String,
    pub kind: String,
    pub image: Option<String>,
    pub source_id: SourceId,
    pub category_id: Option<CategoryId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFilter {
    pub source_id: Option<SourceId>,
    pub category_id: Option<CategoryId>,
    pub kind: Option<String>,
    pub query: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub trait ContentProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn find_content(
        &self,
        id: &ContentId,
    ) -> impl Future<Output = Result<Option<ContentItem>, Self::Error>> + Send;

    fn find_content_by_slug(
        &self,
        slug: &str,
    ) -> impl Future<Output = Result<Option<ContentItem>, Self::Error>> + Send;

    fn find_content_by_source_and_slug(
        &self,
        source_id: &SourceId,
        slug: &str,
    ) -> impl Future<Output = Result<Option<ContentItem>, Self::Error>> + Send;

    fn list_content(
        &self,
        filter: ContentFilter,
    ) -> impl Future<Output = Result<Vec<ContentSummary>, Self::Error>> + Send;

    fn search(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> impl Future<Output = Result<Vec<ContentSummary>, Self::Error>> + Send;
}
