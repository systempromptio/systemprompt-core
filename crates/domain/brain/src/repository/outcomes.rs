//! [`OutcomeRepository`]: observed outcome signals per injected fact, one
//! per recall, fact and signal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgExecutor;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::NewOutcome;

#[derive(Debug, Clone)]
pub struct OutcomeRepository {
    pools: Pools,
}

impl OutcomeRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        executor: impl PgExecutor<'_>,
        outcome: &NewOutcome,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "INSERT INTO brain_outcomes (id, recall_id, fact_id, signal, value, observed_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (recall_id, fact_id, signal) DO NOTHING",
            uuid::Uuid::new_v4().to_string(),
            outcome.recall_id.as_str(),
            outcome.fact_id.as_str(),
            outcome.signal,
            outcome.value,
            outcome.observed_at,
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn record_detached(&self, outcome: &NewOutcome) -> Result<bool, RepositoryError> {
        Self::record(&*self.pools.write, outcome).await
    }

    pub async fn sum_for_fact(
        &self,
        fact_id: &BrainFactId,
        signal: &str,
    ) -> Result<f64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(value), 0)::float8 AS "sum!" FROM brain_outcomes
               WHERE fact_id = $1 AND signal = $2"#,
            fact_id.as_str(),
            signal,
        )
        .fetch_one(&*self.pools.read)
        .await?)
    }
}
