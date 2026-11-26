//! [`AiUsageReportRepository`]: the AI usage and cost reports over the
//! `ai` member's report views, the implementation of [`AiUsageReport`].
//!
//! [`costs`] holds the platform-wide and per-user spend, [`conversations`]
//! the distinct conversation counts, [`requests`] the request stats, list,
//! trends, overview figures and gateway sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod conversations;
mod cost_breakdowns;
mod costs;
mod requests;

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::ai::{
    AiUsageReport, ContextGroupRow, ContextSummaryRow, CostBreakdownRow, CostSummaryRow,
    CostTrendRow, CostUserBreakdownRow, GatewaySessionListRow, ModelUsageRow, PreviousCostRow,
    RequestListFilter, RequestListRow, RequestMetricsRow, RequestStatsRow, RequestTrendRow,
};

#[must_use]
#[derive(Debug, Clone)]
pub struct AiUsageReportRepository {
    pool: Arc<PgPool>,
}

impl AiUsageReportRepository {
    pub fn new(db: &DbPool) -> Self {
        Self { pool: db.pool() }
    }
}

#[async_trait]
impl AiUsageReport for AiUsageReportRepository {
    async fn summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError> {
        Self::summary(self, start, end).await
    }

    async fn previous_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError> {
        Self::previous_cost(self, start, end).await
    }

    async fn breakdown_by_model(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
        Self::breakdown_by_model(self, start, end, limit).await
    }

    async fn breakdown_by_provider(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
        Self::breakdown_by_provider(self, start, end, limit).await
    }

    async fn breakdown_by_billing_source(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
        Self::breakdown_by_billing_source(self, start, end, limit).await
    }

    async fn breakdown_by_agent(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
        Self::breakdown_by_agent(self, start, end, limit).await
    }

    async fn breakdown_by_user(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostUserBreakdownRow>, RepositoryError> {
        Self::breakdown_by_user(self, start, end, limit).await
    }

    async fn cost_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CostTrendRow>, RepositoryError> {
        Self::cost_trends(self, start, end).await
    }

    async fn summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow, RepositoryError> {
        Self::summary_for_user(self, user_id, start, end).await
    }

    async fn previous_cost_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError> {
        Self::previous_cost_for_user(self, user_id, start, end).await
    }

    async fn breakdown_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>, RepositoryError> {
        Self::breakdown_by_model_for_user(self, user_id, start, end, limit).await
    }

    async fn context_summary_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<ContextSummaryRow, RepositoryError> {
        Self::context_summary_for_user(self, user_id, start, end).await
    }

    async fn contexts_by_model_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError> {
        Self::contexts_by_model_for_user(self, user_id, start, end, limit).await
    }

    async fn contexts_by_agent_for_user(
        &self,
        user_id: &UserId,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ContextGroupRow>, RepositoryError> {
        Self::contexts_by_agent_for_user(self, user_id, start, end, limit).await
    }

    async fn request_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        model_filter: Option<&str>,
    ) -> Result<RequestStatsRow, RepositoryError> {
        Self::request_stats(self, start, end, model_filter).await
    }

    async fn model_usage(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ModelUsageRow>, RepositoryError> {
        Self::model_usage(self, start, end, limit).await
    }

    async fn request_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<RequestTrendRow>, RepositoryError> {
        Self::request_trends(self, start, end).await
    }

    async fn list_requests(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filter: &RequestListFilter,
    ) -> Result<Vec<RequestListRow>, RepositoryError> {
        Self::list_requests(self, start, end, filter).await
    }

    async fn request_metrics(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<RequestMetricsRow, RepositoryError> {
        Self::request_metrics(self, start, end).await
    }

    async fn overview_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow, RepositoryError> {
        Self::overview_cost(self, start, end).await
    }

    async fn list_gateway_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<GatewaySessionListRow>, RepositoryError> {
        Self::list_gateway_sessions(self, start, end, limit, user).await
    }
}
