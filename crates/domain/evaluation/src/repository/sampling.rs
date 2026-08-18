//! Candidate selection over the `ai_requests` trace.
//!
//! Sampling excludes `actor_kind = 'job'` rows — judge and replay inference
//! is attributed to a job actor, so without this exclusion each run would
//! sample and grade the previous run's judge prompts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ContextId};

use crate::error::Result;
use crate::models::{CanonicalMessage, SampleFilter, SampleMode, SampledRequest};

struct SampledRow {
    id: String,
    context_id: ContextId,
    provider: String,
    model: String,
    system_prompt_override: Option<String>,
    latency_ms: Option<i32>,
    cost_microdollars: i64,
    created_at: DateTime<Utc>,
    offered_tools: Option<serde_json::Value>,
    prepared_body_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SamplingRepository {
    pool: Arc<PgPool>,
}

impl SamplingRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        Ok(Self {
            pool: db.pool_arc()?,
        })
    }

    pub async fn sample(&self, filter: &SampleFilter) -> Result<Vec<SampledRequest>> {
        let rows = match filter.mode {
            SampleMode::Request => self.sample_requests(filter).await?,
            SampleMode::Conversation => self.sample_conversations(filter).await?,
        };

        let mut sampled = Vec::with_capacity(rows.len());
        for row in rows {
            let id = AiRequestId::new(row.id);
            let (messages, response_text) = self.load_messages(&id).await?;
            sampled.push(SampledRequest {
                ai_request_id: id,
                context_id: row.context_id,
                provider: row.provider,
                model: row.model,
                system_prompt_override: row.system_prompt_override,
                messages,
                response_text,
                offered_tools: row.offered_tools,
                prepared_body_sha256: row.prepared_body_sha256,
                latency_ms: row.latency_ms,
                cost_microdollars: row.cost_microdollars,
                created_at: row.created_at,
            });
        }
        Ok(sampled)
    }

    async fn sample_requests(&self, filter: &SampleFilter) -> Result<Vec<SampledRow>> {
        let rows = sqlx::query_as!(
            SampledRow,
            r#"
            SELECT r.id, r.context_id AS "context_id: ContextId", r.provider AS "provider!", r.model AS "model!",
                   r.system_prompt_override, r.latency_ms, r.cost_microdollars,
                   r.created_at, p.offered_tools, p.prepared_body_sha256
            FROM ai_requests r
            LEFT JOIN ai_request_payloads p ON p.ai_request_id = r.id
            WHERE r.status = 'completed'
              AND r.actor_kind <> 'job'
              AND NOT r.synthetic
              AND ($1::timestamptz IS NULL OR r.created_at >= $1)
              AND ($2::timestamptz IS NULL OR r.created_at < $2)
              AND ($3::text IS NULL OR r.provider = $3)
              AND ($4::text IS NULL OR r.model = $4)
              AND ($5::text[] IS NULL OR r.id = ANY($5))
              AND ($6::text IS NULL OR r.context_id = $6)
            ORDER BY r.created_at DESC
            LIMIT $7
            "#,
            filter.since,
            filter.until,
            filter.provider.as_deref(),
            filter.model.as_deref(),
            filter.ids.as_deref(),
            filter.context_id.as_ref().map(|c| c.as_str()),
            filter.limit
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows)
    }

    async fn sample_conversations(&self, filter: &SampleFilter) -> Result<Vec<SampledRow>> {
        let rows = sqlx::query_as!(
            SampledRow,
            r#"
            SELECT latest.id AS "id!", latest.context_id AS "context_id!: ContextId",
                   latest.provider AS "provider!", latest.model AS "model!",
                   latest.system_prompt_override, latest.latency_ms,
                   latest.cost_microdollars AS "cost_microdollars!",
                   latest.created_at AS "created_at!",
                   latest.offered_tools, latest.prepared_body_sha256
            FROM (
                SELECT DISTINCT ON (r.context_id)
                       r.id, r.context_id, r.provider, r.model,
                       r.system_prompt_override, r.latency_ms, r.cost_microdollars,
                       r.created_at, p.offered_tools, p.prepared_body_sha256
                FROM ai_requests r
                LEFT JOIN ai_request_payloads p ON p.ai_request_id = r.id
                WHERE r.status = 'completed'
                  AND r.actor_kind <> 'job'
                  AND NOT r.synthetic
                  AND ($1::timestamptz IS NULL OR r.created_at >= $1)
                  AND ($2::timestamptz IS NULL OR r.created_at < $2)
                  AND ($3::text IS NULL OR r.provider = $3)
                  AND ($4::text IS NULL OR r.model = $4)
                  AND ($5::text[] IS NULL OR r.id = ANY($5))
                  AND ($6::text IS NULL OR r.context_id = $6)
                ORDER BY r.context_id, r.created_at DESC
            ) latest
            ORDER BY latest.created_at DESC
            LIMIT $7
            "#,
            filter.since,
            filter.until,
            filter.provider.as_deref(),
            filter.model.as_deref(),
            filter.ids.as_deref(),
            filter.context_id.as_ref().map(|c| c.as_str()),
            filter.limit
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows)
    }

    /// Cost as persisted by the audit path, looked up by the provider-facing
    /// `request_id` (the UUID on `AiResponse`), not the row's primary key.
    pub async fn request_cost(&self, request_id: &str) -> Result<i64> {
        let cost = sqlx::query_scalar!(
            "SELECT cost_microdollars FROM ai_requests WHERE request_id = $1",
            request_id
        )
        .fetch_optional(self.pool.as_ref())
        .await?;
        Ok(cost.unwrap_or(0))
    }

    /// Splits the stored transcript into the prompt (everything up to the
    /// last assistant message) and the response (that last assistant message).
    async fn load_messages(
        &self,
        id: &AiRequestId,
    ) -> Result<(Vec<CanonicalMessage>, Option<String>)> {
        let rows = sqlx::query!(
            r#"
            SELECT role, content
            FROM ai_request_messages
            WHERE request_id = $1
            ORDER BY sequence_number
            "#,
            id.as_str()
        )
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut messages: Vec<CanonicalMessage> = rows
            .into_iter()
            .map(|row| CanonicalMessage {
                role: row.role,
                content: row.content,
            })
            .collect();

        let response_text = match messages.last() {
            Some(last) if last.role == "assistant" => messages.pop().map(|m| m.content),
            _ => None,
        };
        Ok((messages, response_text))
    }
}
