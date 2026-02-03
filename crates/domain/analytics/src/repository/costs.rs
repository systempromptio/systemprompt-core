use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::models::cli::{CostBreakdownRow, CostSummaryRow, CostTrendRow, PreviousCostRow};

#[derive(Debug)]
pub struct CostAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl CostAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn get_summary(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<CostSummaryRow> {
        sqlx::query_as!(
            CostSummaryRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_requests!",
                SUM(cost_microdollars)::bigint as "total_cost",
                SUM(tokens_used)::bigint as "total_tokens"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_previous_cost(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<PreviousCostRow> {
        sqlx::query_as!(
            PreviousCostRow,
            r#"
            SELECT SUM(cost_microdollars)::bigint as "cost"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_model(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                model as "name!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                COUNT(*)::bigint as "requests!",
                COALESCE(SUM(tokens_used), 0)::bigint as "tokens!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            GROUP BY model
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_provider(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                provider as "name!",
                COALESCE(SUM(cost_microdollars), 0)::bigint as "cost!",
                COUNT(*)::bigint as "requests!",
                COALESCE(SUM(tokens_used), 0)::bigint as "tokens!"
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            GROUP BY provider
            ORDER BY SUM(cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_breakdown_by_agent(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<CostBreakdownRow>> {
        sqlx::query_as!(
            CostBreakdownRow,
            r#"
            SELECT
                COALESCE(at.agent_name, 'unknown') as "name!",
                COALESCE(SUM(r.cost_microdollars), 0)::bigint as "cost!",
                COUNT(*)::bigint as "requests!",
                COALESCE(SUM(r.tokens_used), 0)::bigint as "tokens!"
            FROM ai_requests r
            LEFT JOIN agent_tasks at ON at.task_id = r.task_id
            WHERE r.created_at >= $1 AND r.created_at < $2
            GROUP BY at.agent_name
            ORDER BY SUM(r.cost_microdollars) DESC NULLS LAST
            LIMIT $3
            "#,
            start,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_costs_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<CostTrendRow>> {
        sqlx::query_as!(
            CostTrendRow,
            r#"
            SELECT
                created_at as "created_at!",
                cost_microdollars,
                tokens_used
            FROM ai_requests
            WHERE created_at >= $1 AND created_at < $2
            ORDER BY created_at
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
