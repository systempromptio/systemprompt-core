//! Claim queries: `FOR UPDATE SKIP LOCKED` over queued runs, lease renewal
//! and stale-lease reclaim.
//!
//! `claim_queued` takes the oldest due queued run of an origin kind into
//! `preparing` and returns it with its [`Lease`]. `renew_lease` returns
//! [`Fenced::LeaseLost`] once that claim no longer owns the run (reclaimed,
//! released or terminal). `release` hands a run the claimer stops without
//! finishing (a shutdown) back to `queued` with `resume_session_id` set from
//! the host session; its attempt is unchanged, and the cleared claim makes
//! every later write under the old lease a no-op.
//! `reclaim_stale` re-queues an expired run below `max_attempts` with its
//! attempt bumped and `resume_session_id` set from the host session, and
//! fails the rest as `host_crashed`, moving a failed run's A2A task to
//! `Failed` in the same transaction; it returns every reclaimed id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AgentRunId, TaskId};

use super::{AgentRunRepository, Claimed, Fenced, Lease, task_link};
use crate::error::AgentRunsResult;
use crate::outcome::RunStatus;

impl AgentRunRepository {
    pub async fn claim_queued(
        &self,
        origin_kind: &str,
        claimed_by: &str,
        lease: Duration,
    ) -> AgentRunsResult<Option<Claimed>> {
        let claimed = sqlx::query!(
            r#"
            UPDATE agent_runs
            SET status = 'preparing', claimed_by = $2, claimed_at = NOW(),
                lease_until = NOW() + make_interval(secs => $3::double precision)
            WHERE id = (
                SELECT id FROM agent_runs
                WHERE status = 'queued' AND origin_kind = $1
                  AND (not_before IS NULL OR not_before <= NOW())
                ORDER BY created_at
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id AS "id: AgentRunId", attempt,
                      claimed_at AS "claimed_at!: DateTime<Utc>"
            "#,
            origin_kind,
            claimed_by,
            lease.as_secs_f64(),
        )
        .fetch_optional(self.write_pool())
        .await?;
        let Some(claimed) = claimed else {
            return Ok(None);
        };
        let lease = Lease::new(claimed_by.to_owned(), claimed.attempt, claimed.claimed_at);
        Ok(self
            .find_run(&claimed.id)
            .await?
            .map(|row| Claimed { row, lease }))
    }

    pub async fn renew_lease(
        &self,
        id: &AgentRunId,
        lease: &Lease,
        duration: Duration,
    ) -> AgentRunsResult<Fenced> {
        let result = sqlx::query!(
            r#"
            UPDATE agent_runs
            SET lease_until = NOW() + make_interval(secs => $4::double precision)
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $5
              AND status IN ('preparing', 'running')
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            duration.as_secs_f64(),
            lease.claimed_at(),
        )
        .execute(self.write_pool())
        .await?;
        Ok(Fenced::from_rows(result.rows_affected()))
    }

    pub async fn release(&self, id: &AgentRunId, lease: &Lease) -> AgentRunsResult<Fenced> {
        let result = sqlx::query!(
            r#"
            UPDATE agent_runs
            SET status = 'queued',
                request = CASE WHEN host_session_id IS NULL THEN request
                    ELSE jsonb_set(request, '{resume_session_id}', to_jsonb(host_session_id))
                END,
                claimed_by = NULL, claimed_at = NULL, lease_until = NULL, pid = NULL
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $4
              AND status IN ('preparing', 'running')
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            lease.claimed_at(),
        )
        .execute(self.write_pool())
        .await?;
        Ok(Fenced::from_rows(result.rows_affected()))
    }

    pub async fn reclaim_stale(
        &self,
        origin_kind: &str,
        max_attempts: i32,
    ) -> AgentRunsResult<Vec<AgentRunId>> {
        let mut tx = self.write_pool().begin().await?;
        let rows = sqlx::query!(
            r#"
            WITH stale AS (
                SELECT id, attempt < $2 AS retry FROM agent_runs
                WHERE origin_kind = $1 AND status IN ('preparing', 'running')
                  AND lease_until < NOW()
                FOR UPDATE SKIP LOCKED
            )
            UPDATE agent_runs r
            SET status = CASE WHEN s.retry THEN 'queued' ELSE 'failed' END,
                attempt = CASE WHEN s.retry THEN r.attempt + 1 ELSE r.attempt END,
                request = CASE WHEN s.retry THEN jsonb_set(
                    CASE WHEN r.host_session_id IS NULL THEN r.request
                         ELSE jsonb_set(r.request, '{resume_session_id}', to_jsonb(r.host_session_id))
                    END,
                    '{attempt}', to_jsonb(r.attempt + 1)) ELSE r.request END,
                error_class = CASE WHEN s.retry THEN NULL ELSE 'host_crashed' END,
                error = CASE WHEN s.retry THEN NULL ELSE 'lease expired on the final attempt' END,
                finished_at = CASE WHEN s.retry THEN NULL ELSE NOW() END,
                claimed_by = NULL, claimed_at = NULL, lease_until = NULL, pid = NULL
            FROM stale s
            WHERE r.id = s.id
            RETURNING r.id AS "id: AgentRunId", r.status, r.task_id AS "task_id?: TaskId"
            "#,
            origin_kind,
            max_attempts,
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut ids = Vec::with_capacity(rows.len());
        for row in rows {
            if row.status == "failed"
                && let Some(task_id) = &row.task_id
            {
                task_link::transition_task(&mut tx, task_id, RunStatus::Failed).await?;
            }
            ids.push(row.id);
        }
        tx.commit().await?;
        Ok(ids)
    }
}
