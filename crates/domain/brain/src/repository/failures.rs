//! [`FailureRepository`]: one row per failed pipeline step, counted per
//! event so the consumer can abandon an event after its last attempt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgExecutor;
use systemprompt_identifiers::{EventOutboxId, PipelineVersion};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::extractions::saturating_i32;
use crate::models::NewFailure;

#[derive(Debug, Clone)]
pub struct FailureRepository {
    pools: Pools,
}

impl FailureRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        executor: impl PgExecutor<'_>,
        failure: &NewFailure,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_failures (id, outbox_id, pipeline_version, stage, reason, attempt)
             VALUES ($1, $2, $3, $4, $5, $6)",
            uuid::Uuid::new_v4().to_string(),
            failure.outbox_id.as_str(),
            failure
                .pipeline_version
                .as_ref()
                .map(PipelineVersion::as_str),
            failure.stage,
            failure.reason,
            saturating_i32(failure.attempt),
        )
        .execute(executor)
        .await?;
        Ok(())
    }

    pub async fn count_for_outbox(
        executor: impl PgExecutor<'_>,
        outbox_id: &EventOutboxId,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM brain_failures WHERE outbox_id = $1"#,
            outbox_id.as_str(),
        )
        .fetch_one(executor)
        .await?)
    }

    pub async fn record_detached(&self, failure: &NewFailure) -> Result<(), RepositoryError> {
        Self::record(&*self.pools.write, failure).await
    }
}
