//! Read-only access to the `ai_requests` trace for domains that never own it.
//!
//! The evaluation domain samples completed production requests, verifies the
//! audit trail of an execution, and settles budget reservations against the
//! recorded usage; the request rows belong to the AI domain, so every one of
//! those reads goes through [`AiRequestTrace`], implemented by the AI domain
//! and held as `Arc<dyn AiRequestTrace>` — hence `#[async_trait]`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{AiRequestId, ContextId, ModelId, ProviderId, SessionId, UserId};

use super::AiProviderResult;

/// How completed rows are grouped when sampled.
///
/// `Request` samples every completed row independently; `Conversation`
/// samples one transcript per `context_id` — the latest completed row, whose
/// stored messages already carry the whole conversation-so-far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceSampleMode {
    #[default]
    Request,
    Conversation,
}

/// Selection over completed, non-synthetic, user-attributed requests.
///
/// Rows attributed to a `job` actor are always excluded: judge and replay
/// inference is attributed to a job, so without the exclusion each run would
/// sample the previous run's judge prompts.
#[derive(Debug, Clone, Default)]
pub struct TraceSampleFilter {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub provider: Option<ProviderId>,
    pub model: Option<ModelId>,
    pub ids: Option<Vec<AiRequestId>>,
    pub context_id: Option<ContextId>,
    pub mode: TraceSampleMode,
    pub limit: i64,
}

impl TraceSampleFilter {
    #[must_use]
    pub fn with_limit(limit: i64) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    #[must_use]
    pub fn provider(mut self, provider: ProviderId) -> Self {
        self.provider = Some(provider);
        self
    }

    #[must_use]
    pub fn model(mut self, model: ModelId) -> Self {
        self.model = Some(model);
        self
    }

    #[must_use]
    pub fn ids(mut self, ids: Vec<AiRequestId>) -> Self {
        self.ids = Some(ids);
        self
    }

    #[must_use]
    pub fn context_id(mut self, context_id: ContextId) -> Self {
        self.context_id = Some(context_id);
        self
    }

    #[must_use]
    pub const fn mode(mut self, mode: TraceSampleMode) -> Self {
        self.mode = mode;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceMessage {
    pub role: String,
    pub content: String,
}

/// A completed request hydrated with its stored conversation turns.
///
/// `messages` excludes the final assistant turn, which is surfaced as
/// `response_text`; `offered_tools` is the provider tool schema list as it
/// was sent on the wire.
#[derive(Debug, Clone)]
pub struct TraceSample {
    pub ai_request_id: AiRequestId,
    pub context_id: ContextId,
    pub provider: ProviderId,
    pub model: ModelId,
    pub system_prompt_override: Option<String>,
    pub messages: Vec<TraceMessage>,
    pub response_text: Option<String>,
    // JSON: provider tool schemas are an MCP/provider protocol boundary
    pub offered_tools: Option<serde_json::Value>,
    pub prepared_body_sha256: Option<String>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceRequestStatus {
    Pending,
    Completed,
    Failed,
    Rejected,
    Other,
}

impl TraceRequestStatus {
    #[must_use]
    pub fn parse(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "rejected" => Self::Rejected,
            _ => Self::Other,
        }
    }
}

/// Recorded usage of one request, scoped to the owner that asked.
///
/// A request is settled once the provider call finished and its spend was
/// recorded: `completed_at` is set and the row is either `Completed` or
/// carries `accounting_failed_at` (the spend was recorded but a later
/// accounting step failed, which never replaces the settled cost).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRequestUsage {
    pub request_id: AiRequestId,
    pub session_id: Option<SessionId>,
    pub status: TraceRequestStatus,
    pub completed_at: Option<DateTime<Utc>>,
    pub accounting_failed_at: Option<DateTime<Utc>>,
    pub cost_microdollars: i64,
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub tool_calls: u64,
}

impl TraceRequestUsage {
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.completed_at.is_some()
            && (self.status == TraceRequestStatus::Completed || self.accounting_failed_at.is_some())
    }
}

/// Reads over the AI request trace on behalf of another domain.
///
/// Every usage read is scoped to `owner`: a request another user made is
/// reported as absent, never as someone else's usage. Held as
/// `Arc<dyn AiRequestTrace>` by the evaluation repositories, hence
/// `#[async_trait]`.
#[async_trait]
pub trait AiRequestTrace: Send + Sync {
    async fn sample(&self, filter: &TraceSampleFilter) -> AiProviderResult<Vec<TraceSample>>;

    async fn find_usage(
        &self,
        owner: &UserId,
        request: &AiRequestId,
    ) -> AiProviderResult<Option<TraceRequestUsage>>;

    async fn list_usage(
        &self,
        owner: &UserId,
        requests: &[AiRequestId],
    ) -> AiProviderResult<Vec<TraceRequestUsage>>;
}

pub type DynAiRequestTrace = Arc<dyn AiRequestTrace>;
