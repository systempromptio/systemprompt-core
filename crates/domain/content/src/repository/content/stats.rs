//! Authoritative content counts for behavioral classification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
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
}
