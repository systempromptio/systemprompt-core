//! Idempotent queueing for API callers.
//!
//! A run queued with an idempotency key is unique per `origin_kind`. A
//! second enqueue with the same key and the same request digest answers the
//! existing run; the same key with a different digest is a conflict, so a
//! caller can retry safely but never rebind a key to new work. The digest
//! leaves out `workspace`, which the server assigns per run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sha2::{Digest, Sha256};
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId};

use super::{AgentRunRepository, NewAgentRun};
use crate::error::AgentRunsResult;
use crate::request::RunRequest;

/// What an idempotent enqueue did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Enqueued {
    Created(AgentRunId),
    Existing(AgentRunId),
    Conflict(AgentRunId),
}

pub fn request_digest(request: &RunRequest) -> AgentRunsResult<String> {
    let mut caller_owned = request.clone();
    caller_owned.workspace = std::path::PathBuf::new();
    let bytes = serde_json::to_vec(&caller_owned)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

impl AgentRunRepository {
    pub async fn enqueue_idempotent(
        &self,
        run: NewAgentRun<'_>,
        idempotency_key: &str,
    ) -> AgentRunsResult<Enqueued> {
        let digest = request_digest(run.request)?;
        let request = serde_json::to_value(run.request)?;
        let inserted = sqlx::query_scalar!(
            r#"
            INSERT INTO agent_runs
                (id, origin_kind, origin_id, user_id, host, status, request, attempt, not_before,
                 task_id, context_id, idempotency_key, request_digest)
            VALUES ($1, $2, $3, $4, $5, 'queued', $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (origin_kind, idempotency_key) WHERE idempotency_key IS NOT NULL
            DO NOTHING
            RETURNING id AS "id: AgentRunId"
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
            idempotency_key,
            digest,
        )
        .fetch_optional(self.write_pool())
        .await?;
        if let Some(id) = inserted {
            return Ok(Enqueued::Created(id));
        }
        let existing = sqlx::query!(
            r#"
            SELECT id AS "id: AgentRunId", request_digest
            FROM agent_runs
            WHERE origin_kind = $1 AND idempotency_key = $2 AND user_id = $3
            "#,
            run.origin_kind,
            idempotency_key,
            run.user_id.as_str(),
        )
        .fetch_optional(self.write_pool())
        .await?;
        Ok(match existing {
            Some(row) if row.request_digest.as_deref() == Some(digest.as_str()) => {
                Enqueued::Existing(row.id)
            },
            Some(row) => Enqueued::Conflict(row.id),
            None => Enqueued::Conflict(run.id.clone()),
        })
    }
}
