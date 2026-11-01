//! [`DisputeRepository`]: conflicting candidates raised against existing
//! facts; a dispute never overwrites the fact, a person resolves it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{BrainDisputeId, BrainFactId, UserId};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{DisputeStatus, NewDispute};

#[derive(Debug, Clone)]
pub struct DisputeRepository {
    pools: Pools,
}

impl DisputeRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn open(
        conn: &mut PgConnection,
        dispute: &NewDispute,
    ) -> Result<BrainDisputeId, RepositoryError> {
        let id = BrainDisputeId::generate();
        sqlx::query!(
            "INSERT INTO brain_disputes (id, fact_id, candidate, similarity, reason)
             VALUES ($1, $2, $3, $4, $5)",
            id.as_str(),
            dispute.fact_id.as_str(),
            serde_json::to_value(&dispute.candidate)?,
            dispute.similarity,
            dispute.reason,
        )
        .execute(conn)
        .await?;
        Ok(id)
    }

    pub async fn resolve(
        conn: &mut PgConnection,
        id: &BrainDisputeId,
        by: &UserId,
        resolution: &str,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "UPDATE brain_disputes
             SET status = $2, resolution = $3, resolved_by = $4, resolved_at = now()
             WHERE id = $1 AND status = 'open'",
            id.as_str(),
            DisputeStatus::Resolved.as_str(),
            resolution,
            by.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn open_for_fact(
        executor: impl PgExecutor<'_>,
        fact_id: &BrainFactId,
    ) -> Result<Vec<BrainDisputeId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainDisputeId" FROM brain_disputes
               WHERE fact_id = $1 AND status = 'open' ORDER BY opened_at, id"#,
            fact_id.as_str(),
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn count_open(&self) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM brain_disputes WHERE status = 'open'"#
        )
        .fetch_one(&*self.pools.read)
        .await?)
    }
}
