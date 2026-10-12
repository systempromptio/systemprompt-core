//! [`AgentRunRow`]: one `agent_runs` row as decoded, and the reads
//! that return it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ApiKeyId, AgentRunId, HostSessionId, SessionId, UserId};

use super::AgentRunRepository;
use crate::error::AgentRunsResult;

/// One `agent_runs` row. `host`, `status` and `error_class` stay
/// their column text here; [`crate::HostKind`], [`crate::RunStatus`] and
/// [`crate::ErrorClass`] parse them.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentRunRow {
    pub id: AgentRunId,
    pub origin_kind: String,
    pub origin_id: String,
    pub user_id: UserId,
    pub host: String,
    pub status: String,
    // JSON: the stored `RunRequest`, decoded by the claim path.
    pub request: serde_json::Value,
    pub attempt: i32,
    pub not_before: Option<DateTime<Utc>>,
    pub claimed_by: Option<String>,
    pub lease_until: Option<DateTime<Utc>>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub run_dir: Option<String>,
    pub gateway_session_id: Option<SessionId>,
    pub api_key_id: Option<ApiKeyId>,
    pub host_session_id: Option<HostSessionId>,
    pub pid: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub error_class: Option<String>,
    pub turns: Option<i32>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cost_microdollars: Option<i64>,
    // JSON: the host's structured answer.
    pub structured_output: Option<serde_json::Value>,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
    // JSON: the workspace diff summary.
    pub diff_summary: Option<serde_json::Value>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentRunRepository {
    pub async fn find_run(&self, id: &AgentRunId) -> AgentRunsResult<Option<AgentRunRow>> {
        let row = sqlx::query_as!(
            AgentRunRow,
            r#"
            SELECT id AS "id: AgentRunId", origin_kind, origin_id,
                   user_id AS "user_id: UserId", host, status, request, attempt,
                   not_before, claimed_by, lease_until, claimed_at, run_dir,
                   gateway_session_id AS "gateway_session_id: SessionId",
                   api_key_id AS "api_key_id: ApiKeyId",
                   host_session_id AS "host_session_id: HostSessionId",
                   pid, started_at, finished_at, duration_ms, error_class, turns,
                   input_tokens, output_tokens, cost_microdollars, structured_output,
                   exit_code, error, diff_summary, cancel_requested_at, created_at, updated_at
            FROM agent_runs
            WHERE id = $1
            "#,
            id.as_str(),
        )
        .fetch_optional(self.write_pool())
        .await?;
        Ok(row)
    }

    pub async fn list_runs_by_origin(
        &self,
        origin_kind: &str,
        origin_id: &str,
    ) -> AgentRunsResult<Vec<AgentRunRow>> {
        let rows = sqlx::query_as!(
            AgentRunRow,
            r#"
            SELECT id AS "id: AgentRunId", origin_kind, origin_id,
                   user_id AS "user_id: UserId", host, status, request, attempt,
                   not_before, claimed_by, lease_until, claimed_at, run_dir,
                   gateway_session_id AS "gateway_session_id: SessionId",
                   api_key_id AS "api_key_id: ApiKeyId",
                   host_session_id AS "host_session_id: HostSessionId",
                   pid, started_at, finished_at, duration_ms, error_class, turns,
                   input_tokens, output_tokens, cost_microdollars, structured_output,
                   exit_code, error, diff_summary, cancel_requested_at, created_at, updated_at
            FROM agent_runs
            WHERE origin_kind = $1 AND origin_id = $2
            ORDER BY created_at
            "#,
            origin_kind,
            origin_id,
        )
        .fetch_all(self.write_pool())
        .await?;
        Ok(rows)
    }
}
