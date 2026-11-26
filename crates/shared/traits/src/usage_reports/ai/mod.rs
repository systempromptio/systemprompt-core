//! The AI usage and cost report port.
//!
//! It holds the reads of the `ai` member's report views (`report_ai_requests`,
//! `report_ai_usage_hourly`, `report_ai_context_hourly`) behind `analytics
//! costs`, `analytics requests`, the overview's request and cost figures and
//! the gateway session list.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod rows;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use crate::RepositoryError;

pub use rows::{
    ContextGroupRow, ContextSummaryRow, CostBreakdownRow, CostSummaryRow, CostTrendRow,
    CostUserBreakdownRow, GatewaySessionListRow, ModelUsageRow, PreviousCostRow, RequestListFilter,
    RequestListRow, RequestMetricsRow, RequestStatsRow, RequestTrendRow,
};

/// Reads AI usage and cost over the `ai` member's report views; every window
/// is half-open `[start, end)` and rollup reads widen `start` to its UTC
/// hour.
///
/// The `ai` member provides it as [`DynAiUsageReport`]; a composition
/// without that member has no port and the reports say so. `#[async_trait]`
/// keeps the trait dyn-compatible for that `Arc<dyn _>`.
#[async_trait]
pub trait AiUsageReport: Send + Sync {
    async fn summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError>;

    async fn previous_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError>;

    async fn breakdown_by_model(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError>;

    async fn breakdown_by_provider(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError>;

    async fn breakdown_by_billing_source(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError>;

    async fn breakdown_by_agent(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError>;

    async fn breakdown_by_user(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostUserBreakdownRow>, RepositoryError>;

    async fn cost_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CostTrendRow>, RepositoryError>;

    async fn summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError>;

    async fn previous_cost_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError>;

    async fn breakdown_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError>;

    async fn context_summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ContextSummaryRow, RepositoryError>;

    async fn contexts_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError>;

    async fn contexts_by_agent_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError>;

    async fn request_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        model_filter: Option<&str>,
    ) -> Result<RequestStatsRow, RepositoryError>;

    async fn model_usage(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ModelUsageRow>, RepositoryError>;

    async fn request_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RequestTrendRow>, RepositoryError>;

    async fn list_requests(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filter: &RequestListFilter,
    ) -> Result<Vec<RequestListRow>, RepositoryError>;

    async fn request_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RequestMetricsRow, RepositoryError>;

    async fn overview_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError>;

    async fn list_gateway_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<GatewaySessionListRow>, RepositoryError>;
}

pub type DynAiUsageReport = Arc<dyn AiUsageReport>;
