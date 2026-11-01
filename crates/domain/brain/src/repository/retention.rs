//! [`RetentionRepository`]: bounded deletes the retention job runs in
//! batches, and the facts whose retention has run out. Expired facts are
//! returned rather than retired here, so the job retires each through
//! [`super::FactRepository::apply`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::Pools;

#[derive(Debug, Clone)]
pub struct RetentionRepository {
    pools: Pools,
}

impl RetentionRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn delete_retired_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_facts WHERE id IN (
                 SELECT id FROM brain_facts
                 WHERE status = 'retired' AND NOT legal_hold AND updated_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn expired(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<BrainFactId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE retention_until < $1 AND NOT legal_hold
                 AND status NOT IN ('superseded', 'retired')
               ORDER BY retention_until, id LIMIT $2"#,
            now,
            limit,
        )
        .fetch_all(&*self.pools.read)
        .await?)
    }

    pub async fn purge_event_results_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_event_results WHERE ctid IN (
                 SELECT ctid FROM brain_event_results WHERE claimed_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_failures_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_failures WHERE ctid IN (
                 SELECT ctid FROM brain_failures WHERE created_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_extractions_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_extractions WHERE ctid IN (
                 SELECT ctid FROM brain_extractions WHERE created_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_outcomes_before(
        &self,
        cutoff: DateTime<Utc>,
        batch: i64,
    ) -> Result<u64, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM brain_outcomes WHERE ctid IN (
                 SELECT ctid FROM brain_outcomes WHERE created_at < $1 LIMIT $2)",
            cutoff,
            batch,
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected())
    }
}
