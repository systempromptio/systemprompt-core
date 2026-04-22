use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

use crate::models::cli::{ConversationListRow, TimestampRow};

#[derive(Debug)]
pub struct ConversationAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl ConversationAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn list_conversations(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<ConversationListRow>> {
        sqlx::query_as!(
            ConversationListRow,
            r#"
            WITH agent_convs AS (
                SELECT
                    uc.context_id,
                    uc.name,
                    (SELECT COUNT(*) FROM agent_tasks at WHERE at.context_id = uc.context_id)::bigint as task_count,
                    (SELECT COUNT(*) FROM task_messages tm
                     JOIN agent_tasks at ON at.task_id = tm.task_id
                     WHERE at.context_id = uc.context_id)::bigint as message_count,
                    uc.created_at,
                    uc.updated_at
                FROM user_contexts uc
                WHERE uc.created_at >= $1 AND uc.created_at < $2
            ),
            gateway_convs AS (
                SELECT
                    ar.session_id as context_id,
                    NULL::text as name,
                    0::bigint as task_count,
                    COUNT(arm.id)::bigint as message_count,
                    MIN(ar.created_at) as created_at,
                    MAX(ar.created_at) as updated_at
                FROM ai_requests ar
                LEFT JOIN ai_request_messages arm ON arm.request_id = ar.id
                WHERE ar.task_id IS NULL
                  AND ar.session_id IS NOT NULL
                  AND ar.created_at >= $1 AND ar.created_at < $2
                  AND NOT EXISTS (
                      SELECT 1 FROM user_contexts uc2 WHERE uc2.context_id = ar.session_id
                  )
                GROUP BY ar.session_id
            )
            SELECT
                context_id as "context_id!",
                name,
                task_count as "task_count!",
                message_count as "message_count!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM (
                SELECT * FROM agent_convs
                UNION ALL
                SELECT * FROM gateway_convs
            ) combined
            ORDER BY updated_at DESC
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

    pub async fn get_context_count(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM user_contexts WHERE created_at >= $1 AND created_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_task_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<(i64, Option<f64>)> {
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*)::bigint as "count!", AVG(execution_time_ms)::float8 as avg_time
            FROM agent_tasks
            WHERE started_at >= $1 AND started_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok((row.count, row.avg_time))
    }

    pub async fn get_message_count(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM task_messages WHERE created_at >= $1 AND created_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(count)
    }

    pub async fn get_context_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimestampRow>> {
        sqlx::query_as!(
            TimestampRow,
            r#"
            SELECT created_at as "timestamp!"
            FROM user_contexts
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_task_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimestampRow>> {
        sqlx::query_as!(
            TimestampRow,
            r#"
            SELECT started_at as "timestamp!"
            FROM agent_tasks
            WHERE started_at >= $1 AND started_at < $2
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_message_timestamps(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TimestampRow>> {
        sqlx::query_as!(
            TimestampRow,
            r#"
            SELECT created_at as "timestamp!"
            FROM task_messages
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }
}
