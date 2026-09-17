//! Transactional, replay-safe settlement of a gateway request's terminal
//! outcome — usage, cost, response payload, assistant text and tool calls for
//! a completion, or the error for a failure — under an owner check.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use crate::repository::ai_request_payloads::UpsertPayloadParams;
use sqlx::{Postgres, Transaction};
use systemprompt_identifiers::{AiRequestId, AiToolCallId, UserId};

use super::AiRequestRepository;

#[derive(Debug, Clone, Copy, Default)]
pub struct SettlementUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
    pub reasoning_tokens: u32,
    pub tokens_used: u32,
}

#[derive(Debug, Clone)]
pub struct SettledToolCall {
    pub id: AiToolCallId,
    pub name: String,
    pub input: String,
}

#[derive(Debug)]
pub struct SettleCompletion<'a> {
    pub usage: SettlementUsage,
    pub cost_microdollars: i64,
    pub latency_ms: i32,
    pub upstream_latency_ms: Option<i32>,
    pub finish_reason: Option<&'a str>,
    pub payload: UpsertPayloadParams<'a>,
    pub assistant_text: Option<&'a str>,
    pub tool_calls: &'a [SettledToolCall],
}

#[derive(Debug)]
pub enum SettlementOutcome<'a> {
    Completed(SettleCompletion<'a>),
    Failed { error: &'a str },
}

impl AiRequestRepository {
    pub async fn mark_accounting_failed(
        &self,
        request_id: &AiRequestId,
        owner: &UserId,
        error: &str,
    ) -> Result<(), RepositoryError> {
        if error.is_empty() || error.len() > 4096 {
            return Err(RepositoryError::SettlementConflict {
                request_id: request_id.clone(),
                reason: "accounting failure must contain bounded diagnostic evidence".to_owned(),
            });
        }
        let affected = sqlx::query!(
            "UPDATE ai_requests SET status='failed', accounting_failed_at=COALESCE(accounting_failed_at,CURRENT_TIMESTAMP), accounting_error=COALESCE(accounting_error,$3), error_message=COALESCE(accounting_error,$3), updated_at=CASE WHEN accounting_failed_at IS NULL THEN CURRENT_TIMESTAMP ELSE updated_at END WHERE id=$1 AND user_id=$2 AND (accounting_error IS NULL OR accounting_error=$3)",
            request_id.as_str(), owner.as_str(), error
        ).execute(self.write_pool()).await?.rows_affected();
        if affected != 1 {
            return Err(RepositoryError::SettlementConflict {
                request_id: request_id.clone(),
                reason: "accounting failure owner, request or retained diagnostic differs"
                    .to_owned(),
            });
        }
        Ok(())
    }

    #[must_use = "this returns a Result that should not be ignored"]
    pub async fn settle(
        &self,
        request_id: &AiRequestId,
        owner: &UserId,
        outcome: SettlementOutcome<'_>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.write_pool().begin().await?;
        let stored_owner = sqlx::query_scalar!(
            "SELECT user_id FROM ai_requests WHERE id = $1 FOR UPDATE",
            request_id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| RepositoryError::SettlementConflict {
            request_id: request_id.clone(),
            reason: "request row does not exist".to_owned(),
        })?;
        if stored_owner != owner.as_str() {
            return Err(RepositoryError::SettlementConflict {
                request_id: request_id.clone(),
                reason: "settlement owner differs from the request owner".to_owned(),
            });
        }
        match outcome {
            SettlementOutcome::Completed(completion) => {
                settle_completion(&mut tx, request_id, &completion).await?;
            },
            SettlementOutcome::Failed { error } => {
                sqlx::query!(
                    r#"
                    UPDATE ai_requests
                    SET status = 'failed', error_message = $2,
                        completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = $1 AND status <> 'completed' AND accounting_failed_at IS NULL
                    "#,
                    request_id.as_str(),
                    error
                )
                .execute(&mut *tx)
                .await?;
            },
        }
        tx.commit().await?;
        Ok(())
    }
}

async fn settle_completion(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &AiRequestId,
    completion: &SettleCompletion<'_>,
) -> Result<(), RepositoryError> {
    let previous = sqlx::query_scalar!(
        "SELECT response_body_sha256 FROM ai_request_payloads WHERE ai_request_id = $1",
        request_id.as_str()
    )
    .fetch_optional(&mut **tx)
    .await?
    .flatten();
    if let Some(previous) = previous.as_deref()
        && completion.payload.sha256 != Some(previous)
    {
        return Err(RepositoryError::SettlementConflict {
            request_id: request_id.clone(),
            reason: "a different terminal response is already settled".to_owned(),
        });
    }
    if previous.is_none() {
        insert_turn(tx, request_id, completion).await?;
    }
    let payload = &completion.payload;
    sqlx::query!(
        r#"
        INSERT INTO ai_request_payloads (
            ai_request_id, response_body, response_excerpt,
            response_truncated, response_bytes, response_body_sha256,
            created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (ai_request_id) DO UPDATE
        SET response_body = EXCLUDED.response_body,
            response_excerpt = EXCLUDED.response_excerpt,
            response_truncated = EXCLUDED.response_truncated,
            response_bytes = EXCLUDED.response_bytes,
            response_body_sha256 = EXCLUDED.response_body_sha256,
            updated_at = CURRENT_TIMESTAMP
        "#,
        request_id.as_str(),
        payload.body,
        payload.excerpt,
        payload.truncated,
        payload.bytes,
        payload.sha256
    )
    .execute(&mut **tx)
    .await?;
    let usage = completion.usage;
    sqlx::query!(
        r#"
        UPDATE ai_requests
        SET input_tokens = $2, output_tokens = $3, cache_read_tokens = $4,
            cache_creation_tokens = $5, reasoning_tokens = $6, tokens_used = $7,
            cost_microdollars = $8, latency_ms = $9, upstream_latency_ms = $10,
            cache_hit = $11, finish_reason = $12,
            status = CASE WHEN accounting_failed_at IS NULL THEN 'completed' ELSE 'failed' END,
            completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $1
        "#,
        request_id.as_str(),
        tokens(usage.input_tokens)?,
        tokens(usage.output_tokens)?,
        tokens(usage.cache_read_tokens)?,
        tokens(usage.cache_creation_tokens)?,
        tokens(usage.reasoning_tokens)?,
        tokens(usage.tokens_used)?,
        completion.cost_microdollars,
        completion.latency_ms,
        completion.upstream_latency_ms,
        usage.cache_read_tokens > 0,
        completion.finish_reason
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_turn(
    tx: &mut Transaction<'_, Postgres>,
    request_id: &AiRequestId,
    completion: &SettleCompletion<'_>,
) -> Result<(), RepositoryError> {
    for (index, tool) in completion.tool_calls.iter().enumerate() {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_tool_calls (
                request_id, ai_tool_call_id, tool_name, tool_input, sequence_number
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
            request_id.as_str(),
            tool.id.as_str(),
            tool.name,
            tool.input,
            tokens(u32::try_from(index + 1).unwrap_or(u32::MAX))?
        )
        .execute(&mut **tx)
        .await?;
    }
    if let Some(text) = completion.assistant_text {
        sqlx::query!(
            r#"
            INSERT INTO ai_request_messages (request_id, role, content, sequence_number)
            SELECT $1::varchar, 'assistant', $2, COALESCE(MAX(sequence_number), -1) + 1
            FROM ai_request_messages WHERE request_id = $1::varchar
            "#,
            request_id.as_str(),
            text
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn tokens(value: u32) -> Result<i32, RepositoryError> {
    i32::try_from(value).map_err(|error| RepositoryError::InvalidData {
        field: "tokens".to_owned(),
        reason: format!("{value} exceeds the i32 column range: {error}"),
    })
}
