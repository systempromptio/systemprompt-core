//! The data half of `systemprompt admin config migrate --data`.
//!
//! Records how each pre-0.73 provider entry maps onto the catalog
//! (`ai_catalog_legacy_map`), moves stored personal keys onto their
//! credential id, and backfills `ai_requests.credential`/`endpoint` in
//! bounded batches selected through `idx_ai_requests_provider_status`, so a
//! 100M-row table is never rewritten by one statement. Every write is
//! idempotent: a row already carrying its credential is never selected again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_traits::RepositoryError;

/// One legacy entry and the catalog hop it became.
#[derive(Debug, Clone, Copy)]
pub struct LegacyMapRow<'a> {
    pub legacy_provider: &'a str,
    pub provider: &'a str,
    pub endpoint: &'a str,
    pub credential: &'a str,
}

#[derive(Debug, Clone)]
pub struct CatalogMigrationRepository {
    write_pool: Arc<PgPool>,
}

impl CatalogMigrationRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    pub async fn upsert_legacy_map(&self, row: LegacyMapRow<'_>) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"INSERT INTO ai_catalog_legacy_map (legacy_provider, provider, endpoint, credential)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (legacy_provider) DO UPDATE
                   SET provider = EXCLUDED.provider, endpoint = EXCLUDED.endpoint,
                       credential = EXCLUDED.credential"#,
            row.legacy_provider,
            row.provider,
            row.endpoint,
            row.credential,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn update_personal_keys(
        &self,
        legacy_provider: &str,
        credential: &str,
    ) -> Result<u64, RepositoryError> {
        let done = sqlx::query!(
            r#"UPDATE user_provider_credentials AS k SET credential = $2
               WHERE k.credential = $1 AND $1 <> $2
                 AND NOT EXISTS (
                     SELECT 1 FROM user_provider_credentials o
                     WHERE o.user_id = k.user_id AND o.credential = $2
                 )"#,
            legacy_provider,
            credential,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(done.rows_affected())
    }

    pub async fn update_audit_batch(
        &self,
        row: LegacyMapRow<'_>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let done = sqlx::query!(
            r#"UPDATE ai_requests SET credential = $2, endpoint = $3
               WHERE id IN (
                   SELECT id FROM ai_requests
                   WHERE provider = $1 AND credential IS NULL
                     AND (served_provider IS NULL OR served_provider = provider)
                   LIMIT $4
               )"#,
            row.legacy_provider,
            row.credential,
            row.endpoint,
            batch,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(done.rows_affected())
    }

    pub async fn update_served_audit_batch(
        &self,
        row: LegacyMapRow<'_>,
        routed: &[String],
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let done = sqlx::query!(
            r#"UPDATE ai_requests SET credential = $2, endpoint = $3
               WHERE id IN (
                   SELECT id FROM ai_requests
                   WHERE provider = ANY($4) AND served_provider = $1 AND credential IS NULL
                   LIMIT $5
               )"#,
            row.legacy_provider,
            row.credential,
            row.endpoint,
            routed,
            batch,
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(done.rows_affected())
    }
}
