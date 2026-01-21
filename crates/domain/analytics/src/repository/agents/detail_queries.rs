use anyhow::Result;
use chrono::{DateTime, Utc};

use super::AgentAnalyticsRepository;
use crate::models::cli::{AgentErrorRow, AgentHourlyRow, AgentStatusBreakdownRow, AgentSummaryRow};

impl AgentAnalyticsRepository {
    pub async fn agent_exists(
        &self,
        agent_name: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64> {
        let pattern = format!("%{}%", agent_name);
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM agent_tasks WHERE agent_name ILIKE $1 AND started_at >= $2 AND \
             started_at < $3",
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
