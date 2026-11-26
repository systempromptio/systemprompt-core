//! The content rows the audience reports name, read from the
//! `report_markdown_content` view this member owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::content::{ContentAudienceReport, ContentReportItem};

#[derive(Debug, Clone)]
pub struct ContentAudienceReportRepository {
    pool: Arc<PgPool>,
}

impl ContentAudienceReportRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }
}

#[async_trait]
impl ContentAudienceReport for ContentAudienceReportRepository {
    async fn content_items(
        &self,
        ids: &[ContentId],
    ) -> Result<Vec<ContentReportItem>, RepositoryError> {
        sqlx::query_as!(
            ContentReportItem,
            r#"
            SELECT
                mc.id as "id!: ContentId",
                mc.slug as "slug!",
                mc.title as "title!",
                mc.kind as "kind!",
                mc.source_id as "source_id!: SourceId",
                mc.category_id as "category_id?: CategoryId",
                mc.published_at as "published_at!"
            FROM report_markdown_content mc
            WHERE mc.id = ANY($1)
            "#,
            ids as _
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::database)
    }

    async fn source_content(
        &self,
        source_id: &SourceId,
    ) -> Result<Vec<ContentReportItem>, RepositoryError> {
        sqlx::query_as!(
            ContentReportItem,
            r#"
            SELECT
                mc.id as "id!: ContentId",
                mc.slug as "slug!",
                mc.title as "title!",
                mc.kind as "kind!",
                mc.source_id as "source_id!: SourceId",
                mc.category_id as "category_id?: CategoryId",
                mc.published_at as "published_at!"
            FROM report_markdown_content mc
            WHERE mc.source_id = $1
            ORDER BY mc.published_at DESC
            "#,
            source_id as _
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(RepositoryError::database)
    }
}
