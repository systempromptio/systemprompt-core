//! The content audience ranking the prerender orders its popular lists by:
//! a source's items, most distinct human viewers first, then newest first.
//!
//! The viewer counts come from the `DynContentAudienceCounts` the analytics
//! member provides; on a host without it every item counts zero and the
//! ranking is newest first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{ContentId, SourceId};
use systemprompt_traits::usage_reports::content::{
    ContentAudienceReport, DynContentAudienceCounts, rank_by_viewers, source_endpoint_prefix,
};
use systemprompt_traits::{ContentPopularity, RepositoryError};

use crate::repository::ContentAudienceReportRepository;

#[derive(Debug, Clone)]
pub struct ContentPopularityService {
    content: ContentAudienceReportRepository,
    counts: Option<DynContentAudienceCounts>,
}

impl ContentPopularityService {
    pub const fn new(
        content: ContentAudienceReportRepository,
        counts: Option<DynContentAudienceCounts>,
    ) -> Self {
        Self { content, counts }
    }
}

#[async_trait]
impl ContentPopularity for ContentPopularityService {
    async fn popular_content_ids(
        &self,
        source_id: &SourceId,
        days: i32,
        limit: i64,
    ) -> Result<Vec<ContentId>, RepositoryError> {
        let items = self.content.source_content(source_id).await?;
        let viewers = match &self.counts {
            Some(counts) => {
                counts
                    .endpoint_viewers(&source_endpoint_prefix(source_id), days)
                    .await?
            },
            None => Vec::new(),
        };
        let limit = usize::try_from(limit).unwrap_or(0);
        Ok(rank_by_viewers(items, &viewers, limit)
            .into_iter()
            .map(|item| item.id)
            .collect())
    }
}
