
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;

use crate::models::cli::{
    AgentAiStatsRow, AgentErrorRow, AgentHourlyRow, AgentListRow, AgentStatsRow,
    AgentStatusBreakdownRow, AgentSummaryRow, AgentTaskRow,
};

#[derive(Debug)]
pub struct AgentAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl AgentAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn list_agents(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        sort_order: &str,
    ) -> Result<Vec<AgentListRow>> {
        // We need separate queries for each sort order to satisfy sqlx macro requirements
        match sort_order {
            "success_rate" => {
                sqlx::query_as!(
                    AgentListRow,
                    r#"
                    SELECT
                        t.agent_name as "agent_name!",
                        COUNT(*)::bigint as "task_count!",
                        COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                        COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                        COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                        MAX(t.started_at) as "last_active!"
                    FROM agent_tasks t
                    LEFT JOIN ai_requests r ON r.task_id = t.task_id
                    WHERE t.started_at >= $1 AND t.started_at < $2
                      AND t.agent_name IS NOT NULL
                    GROUP BY t.agent_name
                    ORDER BY CASE WHEN COUNT(*) > 0
                        THEN COUNT(*) FILTER (WHERE t.status = 'completed')::float / COUNT(*)::float
                        ELSE 0 END DESC
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
            "cost" => {
                sqlx::query_as!(
                    AgentListRow,
                    r#"
                    SELECT
                        t.agent_name as "agent_name!",
                        COUNT(*)::bigint as "task_count!",
                        COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                        COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                        COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                        MAX(t.started_at) as "last_active!"
                    FROM agent_tasks t
                    LEFT JOIN ai_requests r ON r.task_id = t.task_id
                    WHERE t.started_at >= $1 AND t.started_at < $2
                      AND t.agent_name IS NOT NULL
                    GROUP BY t.agent_name
                    ORDER BY COALESCE(SUM(r.cost_cents), 0) DESC
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
            "last_active" => {
                sqlx::query_as!(
                    AgentListRow,
                    r#"
                    SELECT
                        t.agent_name as "agent_name!",
                        COUNT(*)::bigint as "task_count!",
                        COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                        COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                        COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                        MAX(t.started_at) as "last_active!"
                    FROM agent_tasks t
                    LEFT JOIN ai_requests r ON r.task_id = t.task_id
                    WHERE t.started_at >= $1 AND t.started_at < $2
                      AND t.agent_name IS NOT NULL
                    GROUP BY t.agent_name
                    ORDER BY MAX(t.started_at) DESC
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
            _ => {
                // Default: task_count
                sqlx::query_as!(
                    AgentListRow,
                    r#"
                    SELECT
                        t.agent_name as "agent_name!",
                        COUNT(*)::bigint as "task_count!",
                        COUNT(*) FILTER (WHERE t.status = 'completed')::bigint as "completed_count!",
                        COALESCE(AVG(t.execution_time_ms), 0)::bigint as "avg_execution_time_ms!",
                        COALESCE(SUM(r.cost_cents), 0)::bigint as "total_cost_cents!",
                        MAX(t.started_at) as "last_active!"
                    FROM agent_tasks t
                    LEFT JOIN ai_requests r ON r.task_id = t.task_id
                    WHERE t.started_at >= $1 AND t.started_at < $2
                      AND t.agent_name IS NOT NULL
                    GROUP BY t.agent_name
                    ORDER BY COUNT(*) DESC
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
        }
    }

    pub async fn get_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<AgentStatsRow> {
        if let Some(agent) = agent_filter {
            let pattern = format!("%{}%", agent);
            sqlx::query_as!(
                AgentStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT agent_name)::bigint as "total_agents!",
                    COUNT(*)::bigint as "total_tasks!",
                    COUNT(*) FILTER (WHERE status = 'completed')::bigint as "completed_tasks!",
                    COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed_tasks!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_execution_time_ms!"
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                  AND agent_name ILIKE $3
                "#,
                start,
                end,
                pattern
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                AgentStatsRow,
                r#"
                SELECT
                    COUNT(DISTINCT agent_name)::bigint as "total_agents!",
                    COUNT(*)::bigint as "total_tasks!",
                    COUNT(*) FILTER (WHERE status = 'completed')::bigint as "completed_tasks!",
                    COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed_tasks!",
                    COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_execution_time_ms!"
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                "#,
                start,
                end
            )
            .fetch_one(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn get_ai_stats(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<AgentAiStatsRow> {
        sqlx::query_as!(
            AgentAiStatsRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_ai_requests!",
                COALESCE(SUM(cost_cents), 0)::bigint as "total_cost_cents!"
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

    pub async fn get_tasks_for_trends(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        agent_filter: Option<&str>,
    ) -> Result<Vec<AgentTaskRow>> {
        if let Some(agent) = agent_filter {
            let pattern = format!("%{}%", agent);
            sqlx::query_as!(
                AgentTaskRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status,
                    execution_time_ms
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                  AND agent_name ILIKE $3
                ORDER BY started_at
                "#,
                start,
                end,
                pattern
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        } else {
            sqlx::query_as!(
                AgentTaskRow,
                r#"
                SELECT
                    started_at as "started_at!",
                    status,
                    execution_time_ms
                FROM agent_tasks
                WHERE started_at >= $1 AND started_at < $2
                ORDER BY started_at
                "#,
                start,
                end
            )
            .fetch_all(&*self.pool)
            .await
            .map_err(Into::into)
        }
    }

    pub async fn agent_exists(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let pattern = format!("%{}%", agent_name);
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM agent_tasks WHERE agent_name ILIKE $1 AND started_at >= $2 AND started_at < $3",
        )
        .bind(&pattern)
        .bind(start)
        .bind(end)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_agent_summary(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<AgentSummaryRow> {
        let pattern = format!("%{}%", agent_name);
        sqlx::query_as!(
            AgentSummaryRow,
            r#"
            SELECT
                COUNT(*)::bigint as "total_tasks!",
                COUNT(*) FILTER (WHERE status = 'completed')::bigint as "completed!",
                COUNT(*) FILTER (WHERE status = 'failed')::bigint as "failed!",
                COALESCE(AVG(execution_time_ms)::float8, 0) as "avg_time!"
            FROM agent_tasks
            WHERE agent_name ILIKE $1
              AND started_at >= $2 AND started_at < $3
            "#,
            pattern,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_status_breakdown(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentStatusBreakdownRow>> {
        let pattern = format!("%{}%", agent_name);
        sqlx::query_as!(
            AgentStatusBreakdownRow,
            r#"
            SELECT status as "status!", COUNT(*)::bigint as "status_count!"
            FROM agent_tasks
            WHERE agent_name ILIKE $1
              AND started_at >= $2 AND started_at < $3
            GROUP BY status
            ORDER BY 2 DESC
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_top_errors(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentErrorRow>> {
        let pattern = format!("%{}%", agent_name);
        sqlx::query_as!(
            AgentErrorRow,
            r#"
            SELECT
                COALESCE(
                    SUBSTRING(l.message FROM 1 FOR 100),
                    'Unknown error'
                ) as "error_type",
                COUNT(*)::bigint as "error_count!"
            FROM agent_tasks at
            LEFT JOIN logs l ON l.task_id = at.task_id AND l.level = 'ERROR'
            WHERE at.agent_name ILIKE $1
              AND at.started_at >= $2 AND at.started_at < $3
              AND at.status = 'failed'
            GROUP BY SUBSTRING(l.message FROM 1 FOR 100)
            ORDER BY 2 DESC
            LIMIT 10
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_hourly_distribution(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<AgentHourlyRow>> {
        let pattern = format!("%{}%", agent_name);
        sqlx::query_as!(
            AgentHourlyRow,
            r#"
            SELECT
                EXTRACT(HOUR FROM started_at)::INTEGER as "task_hour!",
                COUNT(*)::bigint as "task_count!"
            FROM agent_tasks
            WHERE agent_name ILIKE $1
              AND started_at >= $2 AND started_at < $3
            GROUP BY EXTRACT(HOUR FROM started_at)
            ORDER BY 1
            "#,
            pattern,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
