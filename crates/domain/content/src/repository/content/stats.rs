//! Authoritative content counts and existence lookups behind
//! [`ContentCatalogStats`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::{LocaleCode, SourceId};
use systemprompt_traits::{ContentCatalogStats, RepositoryError};

use super::ContentRepository;

#[async_trait]
impl ContentCatalogStats for ContentRepository {
    async fn count_public_pages(&self) -> Result<i64, RepositoryError> {
        sqlx::query_scalar!(
            r#"
        SELECT COUNT(*)::BIGINT as "count!"
        FROM markdown_content
        WHERE public = true
        "#
        )
        .fetch_one(&*self.write_pool)
        .await
        .map_err(RepositoryError::database)
    }

    async fn has_content(
        &self,
        source_id: &SourceId,
        slug: &str,
        locale: &LocaleCode,
    ) -> Result<bool, RepositoryError> {
        self.find_by_source_and_slug(source_id, slug, locale)
            .await
            .map(|content| content.is_some())
            .map_err(RepositoryError::database)
    }
}
