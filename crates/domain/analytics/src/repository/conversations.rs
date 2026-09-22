//! Conversation analytics over agent contexts and gateway sessions.
//!
//! [`ConversationAnalyticsRepository`] lists agent-task contexts and
//! task-less gateway AI sessions, and reports task, message, and timestamp
//! counts used to build conversation activity trends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::ContextKind;

use crate::models::reporting::{ConversationListRow, GatewaySessionListRow, TimestampRow};

#[derive(Debug)]
pub struct ConversationAnalyticsRepository {
    pool: Arc<PgPool>,
}

impl ConversationAnalyticsRepository {
    pub fn new(db: &DbPool) -> Result<Self> {
        let pool = db.pool_arc()?;
        Ok(Self { pool })
    }

    pub async fn list_agent_contexts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<ConversationListRow>> {
        let user = user.map(UserId::as_str);
        sqlx::query_as!(
            ConversationListRow,
            r#"
            SELECT
                uc.context_id as "context_id!: systemprompt_identifiers::ContextId",
                uc.user_id as "user_id!: systemprompt_identifiers::UserId",
                uc.name as "name?",
                (SELECT COUNT(*) FROM analytics_report_agent_tasks at WHERE at.context_id = uc.context_id)::bigint as "task_count!",
                (SELECT COUNT(*) FROM analytics_report_task_messages tm
                 JOIN analytics_report_agent_tasks at ON at.task_id = tm.task_id
                 WHERE at.context_id = uc.context_id)::bigint as "message_count!",
                uc.created_at as "created_at!",
                uc.updated_at as "updated_at!"
            FROM analytics_report_user_contexts uc
            WHERE uc.created_at >= $1 AND uc.created_at < $2 AND uc.kind = $4
              AND ($5::text IS NULL OR uc.user_id = $5)
            ORDER BY uc.updated_at DESC
            LIMIT $3
            "#,
            start,
            end,
            limit,
            ContextKind::User.as_str(),
            user
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_gateway_sessions(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<GatewaySessionListRow>> {
        let user = user.map(UserId::as_str);
        sqlx::query_as!(
            GatewaySessionListRow,
            r#"
            SELECT
                ar.session_id as "session_id!: systemprompt_identifiers::SessionId",
                MIN(ar.user_id) as "user_id!: systemprompt_identifiers::UserId",
                COALESCE(SUM(ar.message_count), 0)::bigint as "message_count!",
                MIN(ar.created_at) as "created_at!",
                MAX(ar.created_at) as "updated_at!"
            FROM analytics_report_ai_requests ar
            WHERE ar.task_id IS NULL
              AND ar.session_id IS NOT NULL
              AND ar.created_at >= $1 AND ar.created_at < $2
              AND ($4::text IS NULL OR ar.user_id = $4)
              AND NOT EXISTS (
                  SELECT 1 FROM analytics_report_user_contexts uc2 WHERE uc2.context_id::text = ar.session_id
              )
            GROUP BY ar.session_id
            ORDER BY MAX(ar.created_at) DESC
            LIMIT $3
            "#,
            start,
            end,
            limit,
            user
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn get_context_count(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM analytics_report_user_contexts WHERE created_at >= $1 AND created_at < $2 AND kind = $3"#,
            start,
            end,
            ContextKind::User.as_str()
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
            FROM analytics_report_agent_tasks
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
            r#"SELECT COUNT(*)::bigint as "count!" FROM analytics_report_task_messages WHERE created_at >= $1 AND created_at < $2"#,
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
            FROM analytics_report_user_contexts
            WHERE created_at >= $1 AND created_at < $2 AND kind = $3
            "#,
            start,
            end,
            ContextKind::User.as_str()
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
            FROM analytics_report_agent_tasks
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
            FROM analytics_report_task_messages
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
