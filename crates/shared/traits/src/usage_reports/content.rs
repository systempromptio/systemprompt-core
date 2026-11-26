//! The content-audience report ports.
//!
//! They are the content rows a report names ([`ContentAudienceReport`],
//! provided by the `content` member over its `report_markdown_content` view)
//! and the distinct human viewers per content endpoint
//! ([`ContentAudienceCounts`], provided by the `analytics` member over its
//! event views).
//!
//! Neither member reads the other's views: a report that needs both reads
//! each port and joins them with [`rank_by_viewers`] or by content id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

use crate::repository::RepositoryError;

/// One published content item as a report names it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReportItem {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub kind: String,
    pub source_id: SourceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_id: Option<CategoryId>,
    pub published_at: DateTime<Utc>,
}

/// The distinct human viewers one content endpoint had in a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointViewers {
    pub endpoint: String,
    pub viewers: i64,
}

/// Reads the content rows audience reports name.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// [`DynContentAudienceReport`].
#[async_trait]
pub trait ContentAudienceReport: Send + Sync + std::fmt::Debug {
    async fn content_items(
        &self,
        ids: &[ContentId],
    ) -> Result<Vec<ContentReportItem>, RepositoryError>;

    async fn source_content(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<ContentReportItem>, RepositoryError>;
}

pub type DynContentAudienceReport = Arc<dyn ContentAudienceReport>;

/// Counts the distinct human viewers of each page-view endpoint starting
/// with a prefix over a trailing window of days.
///
/// `#[async_trait]` keeps the trait dyn-compatible: it is held as
/// [`DynContentAudienceCounts`].
#[async_trait]
pub trait ContentAudienceCounts: Send + Sync + std::fmt::Debug {
    async fn endpoint_viewers(
        &self,
        endpoint_prefix: &str,
        days: i32,
    ) -> Result<Vec<EndpointViewers>, RepositoryError>;
}

pub type DynContentAudienceCounts = Arc<dyn ContentAudienceCounts>;

pub fn source_endpoint_prefix(source_id: &SourceId) -> String {
    format!("GET /{source_id}/")
}

pub fn rank_by_viewers(
    mut items: Vec<ContentReportItem>,
    viewers: &[EndpointViewers],
    limit: usize,
) -> Vec<ContentReportItem> {
    let counts: HashMap<&str, i64> = viewers
        .iter()
        .map(|row| (row.endpoint.as_str(), row.viewers))
        .collect();
    let count_of = |item: &ContentReportItem| {
        let endpoint = format!("{}{}", source_endpoint_prefix(&item.source_id), item.slug);
        counts.get(endpoint.as_str()).copied().unwrap_or(0)
    };
    items.sort_by(|a, b| {
        count_of(b)
            .cmp(&count_of(a))
            .then_with(|| b.published_at.cmp(&a.published_at))
    });
    items.truncate(limit);
    items
}
