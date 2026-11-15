//! Content provider traits for blog posts, docs, and other published items,
//! and the audience ranking ([`ContentPopularity`]) the prerender orders by.
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
use systemprompt_identifiers::{CategoryId, ContentId, LocaleCode, SourceId};

/// Authoritative content counts and existence lookups, and the content id a
/// slug names.
///
/// Used by behavioral classification, static-content serving and the
/// analytics and engagement ingestion routes (which attach a content id to an
/// event without depending on the content crate; this lookup is event
/// enrichment and tolerates replication lag). It is `dyn` dispatched, hence
/// `#[async_trait]`. The content extension provides it in
/// the extension state as [`DynContentCatalogStats`].
#[async_trait]
pub trait ContentCatalogStats: Send + Sync + std::fmt::Debug {
    async fn count_public_pages(&self) -> Result<i64, crate::RepositoryError>;

    async fn has_content(
        &self,
        source_id: &SourceId,
        slug: &str,
        locale: &LocaleCode,
    ) -> Result<bool, crate::RepositoryError>;

    async fn content_id_by_slug(
        &self,
        slug: &str,
        locale: &LocaleCode,
    ) -> Result<Option<ContentId>, crate::RepositoryError>;
}

pub type DynContentCatalogStats = std::sync::Arc<dyn ContentCatalogStats>;

/// A content source's items ranked by audience: the ids of the items with the
/// most distinct human viewers over a trailing window of days, most viewed
/// first.
///
/// The content prerender orders its "popular" lists with it. The analytics
/// extension provides it in the extension state as
/// [`DynContentPopularity`]; a host without it renders without popularity
/// ordering. It is `dyn` dispatched, hence `#[async_trait]`.
#[async_trait]
pub trait ContentPopularity: Send + Sync + std::fmt::Debug {
    async fn popular_content_ids(
        &self,
        source_id: &SourceId,
        days: i32,
        limit: i64,
    ) -> Result<Vec<ContentId>, crate::RepositoryError>;
}

pub type DynContentPopularity = std::sync::Arc<dyn ContentPopularity>;

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
