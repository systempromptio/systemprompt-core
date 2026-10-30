//! [`EventResultRepository`]: the idempotency ledger. An event is claimed
//! once per pipeline version; a second claim of the same pair answers
//! `false` and the pipeline skips it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{EventOutboxId, PipelineVersion};
use systemprompt_traits::RepositoryError;

use super::Pools;
use super::extractions::saturating_i32;
use crate::models::EventResult;

#[derive(Debug, Clone)]
pub struct EventResultRepository {
    pools: Pools,
}

impl EventResultRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn claim(
        conn: &mut PgConnection,
        outbox_id: &EventOutboxId,
        pipeline_version: &PipelineVersion,
        event_kind: &str,
    ) -> Result<bool, RepositoryError> {
        let claimed = sqlx::query_scalar!(
            "INSERT INTO brain_event_results (outbox_id, pipeline_version, event_kind)
             VALUES ($1, $2, $3)
             ON CONFLICT (outbox_id, pipeline_version) DO NOTHING
             RETURNING outbox_id",
            outbox_id.as_str(),
            pipeline_version.as_str(),
            event_kind,
        )
        .fetch_optional(conn)
        .await?;
        Ok(claimed.is_some())
    }

    pub async fn finish(
        conn: &mut PgConnection,
        result: &EventResult,
    ) -> Result<(), RepositoryError> {
        let updated = sqlx::query!(
            "UPDATE brain_event_results
             SET outcome = $3, filter = $4, facts_written = $5, finished_at = now()
             WHERE outbox_id = $1 AND pipeline_version = $2",
            result.outbox_id.as_str(),
            result.pipeline_version.as_str(),
            result.outcome.as_str(),
            result.filter,
            saturating_i32(result.facts_written),
        )
        .execute(conn)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(RepositoryError::not_found(
                "brain_event_result",
                result.outbox_id.as_str(),
            ));
        }
        Ok(())
    }

    pub async fn outcome(
        &self,
        outbox_id: &EventOutboxId,
        pipeline_version: &PipelineVersion,
    ) -> Result<Option<String>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            "SELECT outcome FROM brain_event_results
             WHERE outbox_id = $1 AND pipeline_version = $2",
            outbox_id.as_str(),
            pipeline_version.as_str(),
        )
        .fetch_optional(&*self.pools.read)
        .await?
        .flatten())
    }
}
