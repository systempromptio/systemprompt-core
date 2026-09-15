//! `AiRequestTrace` over the request rows this crate owns: sampling of
//! completed production traffic and owner-scoped usage reads for the
//! evaluation domain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{AiRequestId, ContextId, ModelId, ProviderId, SessionId, UserId};
use systemprompt_traits::{
    AiProviderError, AiProviderResult, AiRequestTrace, TraceMessage, TraceRequestStatus,
    TraceRequestUsage, TraceSample, TraceSampleFilter, TraceSampleMode,
};

use super::AiRequestRepository;

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

struct UsageRow {
    id: String,
    session_id: Option<SessionId>,
    status: String,
    completed_at: Option<DateTime<Utc>>,
    accounting_failed_at: Option<DateTime<Utc>>,
    cost_microdollars: i64,
    tokens_used: Option<i32>,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    tool_calls: i64,
}

fn internal(error: &sqlx::Error) -> AiProviderError {
    AiProviderError::Internal(error.to_string())
}

async fn sample_requests(
    pool: &PgPool,
    filter: &TraceSampleFilter,
) -> AiProviderResult<Vec<SampledRow>> {
    let ids = filter.ids.as_ref().map(|ids| {
        ids.iter()
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>()
    });
    sqlx::query_as!(
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
        filter.provider.as_ref().map(ProviderId::as_str),
        filter.model.as_ref().map(ModelId::as_str),
        ids.as_deref(),
        filter.context_id.as_ref().map(ContextId::as_str),
        filter.limit
    )
    .fetch_all(pool)
    .await
    .map_err(|error| internal(&error))
}

async fn sample_conversations(
    pool: &PgPool,
    filter: &TraceSampleFilter,
) -> AiProviderResult<Vec<SampledRow>> {
    let ids = filter.ids.as_ref().map(|ids| {
        ids.iter()
            .map(|id| id.as_str().to_owned())
            .collect::<Vec<_>>()
    });
    sqlx::query_as!(
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
        filter.provider.as_ref().map(ProviderId::as_str),
        filter.model.as_ref().map(ModelId::as_str),
        ids.as_deref(),
        filter.context_id.as_ref().map(ContextId::as_str),
        filter.limit
    )
    .fetch_all(pool)
    .await
    .map_err(|error| internal(&error))
}

async fn load_messages(
    pool: &PgPool,
    id: &AiRequestId,
) -> AiProviderResult<(Vec<TraceMessage>, Option<String>)> {
    let rows = sqlx::query!(
        "SELECT role, content FROM ai_request_messages WHERE request_id = $1 ORDER BY sequence_number",
        id.as_str()
    )
    .fetch_all(pool)
    .await
    .map_err(|error| internal(&error))?;
    let mut messages: Vec<TraceMessage> = rows
        .into_iter()
        .map(|row| TraceMessage {
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

fn usage_from_row(row: UsageRow) -> AiProviderResult<TraceRequestUsage> {
    Ok(TraceRequestUsage {
        request_id: AiRequestId::new(row.id),
        session_id: row.session_id,
        status: TraceRequestStatus::parse(&row.status),
        completed_at: row.completed_at,
        accounting_failed_at: row.accounting_failed_at,
        cost_microdollars: row.cost_microdollars,
        tokens_used: row.tokens_used,
        input_tokens: row.input_tokens,
        output_tokens: row.output_tokens,
        tool_calls: u64::try_from(row.tool_calls)
            .map_err(|_e| AiProviderError::Internal("Negative tool-call count".to_owned()))?,
    })
}

#[async_trait]
impl AiRequestTrace for AiRequestRepository {
    async fn sample(&self, filter: &TraceSampleFilter) -> AiProviderResult<Vec<TraceSample>> {
        let rows = match filter.mode {
            TraceSampleMode::Request => sample_requests(self.pool(), filter).await?,
            TraceSampleMode::Conversation => sample_conversations(self.pool(), filter).await?,
        };
        let mut sampled = Vec::with_capacity(rows.len());
        for row in rows {
            let id = AiRequestId::new(row.id);
            let (messages, response_text) = load_messages(self.pool(), &id).await?;
            sampled.push(TraceSample {
                ai_request_id: id,
                context_id: row.context_id,
                provider: ProviderId::new(row.provider),
                model: ModelId::new(row.model),
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

    async fn find_usage(
        &self,
        owner: &UserId,
        request: &AiRequestId,
    ) -> AiProviderResult<Option<TraceRequestUsage>> {
        let ids = [request.clone()];
        Ok(self.list_usage(owner, &ids).await?.into_iter().next())
    }

    async fn list_usage(
        &self,
        owner: &UserId,
        requests: &[AiRequestId],
    ) -> AiProviderResult<Vec<TraceRequestUsage>> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<String> = requests.iter().map(|id| id.as_str().to_owned()).collect();
        let rows = sqlx::query_as!(
            UsageRow,
            r#"
            SELECT r.id, r.session_id AS "session_id?: SessionId", r.status, r.completed_at, r.accounting_failed_at, r.cost_microdollars,
                   r.tokens_used, r.input_tokens, r.output_tokens,
                   (SELECT count(*) FROM ai_request_tool_calls t WHERE t.request_id = r.id) AS "tool_calls!"
            FROM ai_requests r
            WHERE r.user_id = $1 AND r.id = ANY($2)
            ORDER BY r.created_at
            "#,
            owner.as_str(),
            &ids
        )
        .fetch_all(self.write_pool())
        .await
        .map_err(|error| internal(&error))?;
        rows.into_iter().map(usage_from_row).collect()
    }
}
