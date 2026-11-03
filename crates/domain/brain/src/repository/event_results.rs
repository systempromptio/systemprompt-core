//! [`EventResultRepository`]: the idempotency ledger. An event is claimed
//! once per pipeline version; a second claim of the same pair answers
//! `false` and the pipeline skips it. The `_many` forms claim, finish and
//! unclaim a consumer round's events as sets: a round claims every event
//! it decoded, finishes the ones that completed, and deletes the unfinished
//! claims of the ones it will retry, so a retried event is claimed again.
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

    pub async fn claim_many(
        conn: &mut PgConnection,
        pipeline_version: &PipelineVersion,
        events: &[(EventOutboxId, String)],
    ) -> Result<Vec<EventOutboxId>, RepositoryError> {
        let ids: Vec<String> = events.iter().map(|(id, _)| id.to_string()).collect();
        let kinds: Vec<String> = events.iter().map(|(_, kind)| kind.clone()).collect();
        Ok(sqlx::query_scalar!(
            r#"INSERT INTO brain_event_results (outbox_id, pipeline_version, event_kind)
               SELECT e.id, $1, e.kind FROM unnest($2::text[], $3::text[]) AS e(id, kind)
               ON CONFLICT (outbox_id, pipeline_version) DO NOTHING
               RETURNING outbox_id AS "outbox_id: EventOutboxId""#,
            pipeline_version.as_str(),
            &ids,
            &kinds,
        )
        .fetch_all(conn)
        .await?)
    }

    pub async fn finish_many(
        conn: &mut PgConnection,
        pipeline_version: &PipelineVersion,
        results: &[EventResult],
    ) -> Result<u64, RepositoryError> {
        let ids: Vec<String> = results.iter().map(|r| r.outbox_id.to_string()).collect();
        let outcomes: Vec<String> = results
            .iter()
            .map(|r| r.outcome.as_str().to_owned())
            .collect();
        let filters: Vec<String> = results
            .iter()
            .map(|r| r.filter.clone().unwrap_or_default())
            .collect();
        let written: Vec<i32> = results
            .iter()
            .map(|r| saturating_i32(r.facts_written))
            .collect();
        let updated = sqlx::query!(
            "UPDATE brain_event_results r
             SET outcome = x.outcome, filter = NULLIF(x.filter, ''), facts_written = x.written,
                 finished_at = now()
             FROM unnest($2::text[], $3::text[], $4::text[], $5::int4[])
                  AS x(id, outcome, filter, written)
             WHERE r.outbox_id = x.id AND r.pipeline_version = $1",
            pipeline_version.as_str(),
            &ids,
            &outcomes,
            &filters,
            &written,
        )
        .execute(conn)
        .await?;
        Ok(updated.rows_affected())
    }

    pub async fn unclaim_many(
        conn: &mut PgConnection,
        pipeline_version: &PipelineVersion,
        ids: &[EventOutboxId],
    ) -> Result<u64, RepositoryError> {
        let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
        let deleted = sqlx::query!(
            "DELETE FROM brain_event_results
             WHERE pipeline_version = $1 AND outbox_id = ANY($2::text[]) AND finished_at IS NULL",
            pipeline_version.as_str(),
            &ids,
        )
        .execute(conn)
        .await?;
        Ok(deleted.rows_affected())
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
