//! [`AgentRunRepository`]: the `agent_runs` and `agent_run_events`
//! queries.
//!
//! Row writes for the run lifecycle live here; the claim, lease and reclaim
//! queries are in [`claim`], event inserts in [`events`], row reads in
//! [`rows`]. `insert_run_in_tx` queues inside the caller's transaction so a
//! product can insert its own row atomically; `finish` returns `false` when
//! the run was already terminal, so a second finish never overwrites the
//! first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod claim;
pub mod events;
pub mod rows;

use std::path::Path;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ApiKeyId, AgentRunId, HostSessionId, SessionId, UserId};

pub use rows::AgentRunRow;

use crate::error::AgentRunsResult;
use crate::outcome::RunOutcome;
use crate::request::RunRequest;

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
                (id, origin_kind, origin_id, user_id, host, status, request, attempt, not_before)
            VALUES ($1, $2, $3, $4, $5, 'queued', $6, $7, $8)
            "#,
            run.id.as_str(),
            run.origin_kind,
            run.origin_id,
            run.user_id.as_str(),
            run.request.host.as_str(),
            request,
            i32::try_from(run.request.attempt).unwrap_or(i32::MAX),
            run.not_before,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn update_started(
        &self,
        id: &AgentRunId,
        started: RunStarted<'_>,
    ) -> AgentRunsResult<()> {
        let run_dir = started.run_dir.to_string_lossy();
        sqlx::query!(
            r#"
            UPDATE agent_runs
            SET status = 'running', started_at = NOW(), run_dir = $2, pid = $3,
                gateway_session_id = $4, api_key_id = $5
            WHERE id = $1
            "#,
            id.as_str(),
            run_dir.as_ref(),
            started.pid,
            started.gateway_session_id.map(SessionId::as_str),
            started.api_key_id.map(ApiKeyId::as_str),
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
    }

    pub async fn update_host_session(
        &self,
        id: &AgentRunId,
        host_session_id: &HostSessionId,
    ) -> AgentRunsResult<()> {
        sqlx::query!(
            "UPDATE agent_runs SET host_session_id = $2 WHERE id = $1",
            id.as_str(),
            host_session_id.as_str(),
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(())
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
        outcome: &RunOutcome,
        diff_summary: Option<&serde_json::Value>,
    ) -> AgentRunsResult<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE agent_runs
            SET status = $2, error_class = $3, input_tokens = $4, output_tokens = $5,
                cost_microdollars = $6, turns = $7, duration_ms = $8,
                structured_output = $9, exit_code = $10, error = $11, diff_summary = $12,
                host_session_id = COALESCE($13, host_session_id),
                finished_at = NOW(), lease_until = NULL
            WHERE id = $1
              AND status NOT IN ('succeeded', 'failed', 'cancelled', 'timed_out')
            "#,
            id.as_str(),
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
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
