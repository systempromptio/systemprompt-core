
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;

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
            SELECT
                uc.context_id as "context_id!",
                uc.name,
                (SELECT COUNT(*) FROM agent_tasks at WHERE at.context_id = uc.context_id)::bigint as "task_count!",
                (SELECT COUNT(*) FROM task_messages tm
                 JOIN agent_tasks at ON at.task_id = tm.task_id
                 WHERE at.context_id = uc.context_id)::bigint as "message_count!",
                uc.created_at as "created_at!",
                uc.updated_at as "updated_at!"
            FROM user_contexts uc
            WHERE uc.created_at >= $1 AND uc.created_at < $2
            ORDER BY uc.updated_at DESC
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
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_contexts WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn get_task_stats(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<(i64, Option<f64>)> {
        let row: (i64, Option<f64>) = sqlx::query_as(
            r"
            SELECT COUNT(*), AVG(execution_time_ms)::float8
            FROM agent_tasks
            WHERE started_at >= $1 AND started_at < $2
            ",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_message_count(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_messages WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&*self.pool)
        .await?;
        Ok(row.0)
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
