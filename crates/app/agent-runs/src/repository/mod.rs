//! [`AgentRunRepository`]: the `agent_runs` and `agent_run_events`
//! queries.
//!
//! Row writes for the run lifecycle live here; the claim, lease and reclaim
//! queries are in [`claim`], event inserts in [`events`], row reads in
//! [`rows`]. `insert_run_in_tx` queues inside the caller's transaction so a
//! product can insert its own row atomically. Every write to a claimed run
//! takes the claim's [`Lease`] and returns [`Fenced`]: it applies only while
//! that claim still owns a `preparing` / `running` row, so a reclaimed,
//! released or finished run is never overwritten. When a run carries an A2A
//! `task_id`, `update_started` and `finish` move the task in the same
//! transaction ([`task_link`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod bundle;
pub mod claim;
pub mod events;
pub mod idempotent;
mod lease;
pub mod retention;
pub mod rows;
pub mod task_link;

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{
    AgentRunId, ApiKeyId, ContextId, HostSessionId, SessionId, TaskId, UserId,
};

pub use bundle::{AgentRunRepositories, RunCredentialRepositories};
pub use idempotent::{Enqueued, request_digest};
pub use lease::{Claimed, Fenced, Lease, Released};
pub use rows::AgentRunRow;

use crate::error::AgentRunsResult;
use crate::outcome::{RunOutcome, RunStatus};
use crate::request::RunRequest;
use systemprompt_models::managed::AssetDigest;

#[derive(Debug, Clone)]
pub struct AgentRunRepository {
    write_pool: Arc<PgPool>,
}

/// A run to queue. The `host` column is taken from `request.host`.
#[derive(Debug, Clone, Copy)]
pub struct NewAgentRun<'a> {
    pub id: &'a AgentRunId,
    pub origin_kind: &'a str,
    pub origin_id: &'a str,
    pub user_id: &'a UserId,
    pub request: &'a RunRequest,
    pub not_before: Option<DateTime<Utc>>,
    pub task_id: Option<&'a TaskId>,
    pub context_id: Option<&'a ContextId>,
}

/// What a started host reported, written when the run turns `running`.
#[derive(Debug, Clone, Copy)]
pub struct RunStarted<'a> {
    pub run_dir: &'a Path,
    pub pid: Option<i32>,
    pub gateway_session_id: Option<&'a SessionId>,
    pub api_key_id: Option<&'a ApiKeyId>,
}

impl AgentRunRepository {
    #[must_use]
    pub fn new(db: &DbPool) -> Self {
        Self {
            write_pool: db.write_pool(),
        }
    }

    #[must_use]
    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { write_pool: pool }
    }

    #[must_use]
    pub fn write_pool(&self) -> &PgPool {
        &self.write_pool
    }

    pub async fn insert_run(&self, run: NewAgentRun<'_>) -> AgentRunsResult<()> {
        let mut tx = self.write_pool.begin().await?;
        Self::insert_run_in_tx(&mut tx, run).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn insert_run_in_tx(
        tx: &mut Transaction<'_, Postgres>,
        run: NewAgentRun<'_>,
    ) -> AgentRunsResult<()> {
        let request = serde_json::to_value(run.request)?;
        sqlx::query!(
            r#"
            INSERT INTO agent_runs
                (id, origin_kind, origin_id, user_id, host, status, request, attempt, not_before,
                 task_id, context_id)
            VALUES ($1, $2, $3, $4, $5, 'queued', $6, $7, $8, $9, $10)
            "#,
            run.id.as_str(),
            run.origin_kind,
            run.origin_id,
            run.user_id.as_str(),
            run.request.host.as_str(),
            request,
            i32::try_from(run.request.attempt).unwrap_or(i32::MAX),
            run.not_before,
            run.task_id.map(TaskId::as_str),
            run.context_id.map(ContextId::as_str),
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn record_harness_digest(
        &self,
        id: &AgentRunId,
        lease: &Lease,
        digest: &AssetDigest,
    ) -> AgentRunsResult<Fenced> {
        let result = sqlx::query!(
            r#"
            UPDATE agent_runs SET harness_digest = $4
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $5
              AND status IN ('preparing', 'running')
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            digest.as_str(),
            lease.claimed_at(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(Fenced::from_rows(result.rows_affected()))
    }

    pub async fn update_started(
        &self,
        id: &AgentRunId,
        lease: &Lease,
        started: RunStarted<'_>,
    ) -> AgentRunsResult<Fenced> {
        let run_dir = started.run_dir.to_string_lossy();
        let mut tx = self.write_pool.begin().await?;
        let updated = sqlx::query_scalar!(
            r#"
            UPDATE agent_runs
            SET status = 'running', started_at = NOW(), run_dir = $4, pid = $5,
                gateway_session_id = $6, api_key_id = $7
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $8
              AND status IN ('preparing', 'running')
            RETURNING task_id AS "task_id: TaskId"
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            run_dir.as_ref(),
            started.pid,
            started.gateway_session_id.map(SessionId::as_str),
            started.api_key_id.map(ApiKeyId::as_str),
            lease.claimed_at(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(task_id) = updated else {
            return Ok(Fenced::LeaseLost);
        };
        if let Some(task_id) = task_id {
            task_link::transition_task(&mut tx, &task_id, RunStatus::Running).await?;
        }
        tx.commit().await?;
        Ok(Fenced::Applied)
    }

    pub async fn update_host_session(
        &self,
        id: &AgentRunId,
        lease: &Lease,
        host_session_id: &HostSessionId,
    ) -> AgentRunsResult<Fenced> {
        let result = sqlx::query!(
            r#"
            UPDATE agent_runs SET host_session_id = $4
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $5
              AND status IN ('preparing', 'running')
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            host_session_id.as_str(),
            lease.claimed_at(),
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(Fenced::from_rows(result.rows_affected()))
    }

    pub async fn update_cancel_requested(&self, id: &AgentRunId) -> AgentRunsResult<()> {
        sqlx::query!(
            r#"
            UPDATE agent_runs SET cancel_requested_at = COALESCE(cancel_requested_at, NOW())
            WHERE id = $1
            "#,
            id.as_str(),
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn find_cancel_requested(&self, id: &AgentRunId) -> AgentRunsResult<bool> {
        let requested = sqlx::query_scalar!(
            r#"SELECT cancel_requested_at IS NOT NULL AS "requested!" FROM agent_runs WHERE id = $1"#,
            id.as_str(),
        )
        .fetch_optional(self.write_pool.as_ref())
        .await?;
        Ok(requested.unwrap_or(false))
    }

    // JSON: the workspace diff summary, stored verbatim in `diff_summary`.
    pub async fn finish(
        &self,
        id: &AgentRunId,
        lease: &Lease,
        outcome: &RunOutcome,
        diff_summary: Option<&serde_json::Value>,
    ) -> AgentRunsResult<Fenced> {
        let mut tx = self.write_pool.begin().await?;
        let finished = sqlx::query_scalar!(
            r#"
            UPDATE agent_runs
            SET status = $4, error_class = $5, input_tokens = $6, output_tokens = $7,
                cost_microdollars = $8, turns = $9, duration_ms = $10,
                structured_output = $11, exit_code = $12, error = $13, diff_summary = $14,
                host_session_id = COALESCE($15, host_session_id),
                finished_at = NOW(), lease_until = NULL
            WHERE id = $1 AND claimed_by = $2 AND attempt = $3 AND claimed_at = $16
              AND status IN ('preparing', 'running')
            RETURNING task_id AS "task_id: TaskId"
            "#,
            id.as_str(),
            lease.claimed_by(),
            lease.attempt(),
            outcome.status.as_str(),
            outcome.error_class.as_str(),
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.cost_microdollars,
            outcome.turns,
            outcome.duration_ms,
            outcome.structured_output.as_ref(),
            outcome.exit_code,
            outcome.error.as_deref(),
            diff_summary,
            outcome.session_id.as_ref().map(HostSessionId::as_str),
            lease.claimed_at(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(task_id) = finished else {
            return Ok(Fenced::LeaseLost);
        };
        if let Some(task_id) = task_id {
            task_link::transition_task(&mut tx, &task_id, outcome.status).await?;
        }
        tx.commit().await?;
        Ok(Fenced::Applied)
    }
}
