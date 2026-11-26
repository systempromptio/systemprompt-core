//! Agent conversation contexts: listings, counts and the context, task and
//! message timestamps conversation trends bucket, and one user's most
//! recently active contexts from the AI context rollup.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, UserId};
use systemprompt_models::ContextKind;
use systemprompt_traits::RepositoryError;
use systemprompt_traits::usage_reports::agent::{AgentContextRow, RecentContextRow, TaskStatsRow};

use super::AgentActivityReportRepository;
use crate::models::rows::RecentContextDbRow;

impl AgentActivityReportRepository {
    pub(super) async fn contexts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: i64,
        user: Option<&UserId>,
    ) -> Result<Vec<AgentContextRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            AgentContextRow,
            r#"
            SELECT
                uc.context_id as "context_id!: ContextId",
                uc.user_id as "user_id!: UserId",
                uc.name as "name?",
                (SELECT COUNT(*) FROM report_agent_tasks at WHERE at.context_id = uc.context_id)::bigint as "task_count!",
                (SELECT COUNT(*) FROM report_task_messages tm
                 JOIN report_agent_tasks at ON at.task_id = tm.task_id
                 WHERE at.context_id = uc.context_id)::bigint as "message_count!",
                uc.created_at as "created_at!",
                uc.updated_at as "updated_at!"
            FROM report_user_contexts uc
            WHERE uc.created_at >= $1 AND uc.created_at < $2 AND uc.kind = $4
              AND ($5::text IS NULL OR uc.user_id = $5)
            ORDER BY uc.updated_at DESC
            LIMIT $3
            "#,
            start,
            end,
            limit,
            ContextKind::User.as_str(),
            user as _
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn context_sessions(
        &self,
        session_ids: &[SessionId],
    ) -> Result<Vec<SessionId>, RepositoryError> {
        let ids: Vec<String> = session_ids.iter().map(ToString::to_string).collect();
        let found = sqlx::query_scalar!(
            r#"SELECT context_id as "context_id!" FROM report_user_contexts WHERE context_id = ANY($1)"#,
            &ids
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(found.into_iter().map(SessionId::new).collect())
    }

    pub(super) async fn count_contexts(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM report_user_contexts WHERE created_at >= $1 AND created_at < $2 AND kind = $3"#,
            start,
            end,
            ContextKind::User.as_str()
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    pub(super) async fn tasks_in(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<TaskStatsRow, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT COUNT(*)::bigint as "count!", AVG(execution_time_ms)::float8 as avg_time
            FROM report_agent_tasks
            WHERE started_at >= $1 AND started_at < $2
            "#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?;
        Ok(TaskStatsRow {
            task_count: row.count,
            avg_execution_time_ms: row.avg_time,
        })
    }

    pub(super) async fn count_messages(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint as "count!" FROM report_task_messages WHERE created_at >= $1 AND created_at < $2"#,
            start,
            end
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    pub(super) async fn context_times(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"
            SELECT created_at as "timestamp!"
            FROM report_user_contexts
            WHERE created_at >= $1 AND created_at < $2 AND kind = $3
            "#,
            start,
            end,
            ContextKind::User.as_str()
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn task_times(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"
            SELECT started_at as "timestamp!"
            FROM report_agent_tasks
            WHERE started_at >= $1 AND started_at < $2
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn message_times(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<DateTime<Utc>>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"
            SELECT created_at as "timestamp!"
            FROM report_task_messages
            WHERE created_at >= $1 AND created_at < $2
            "#,
            start,
            end
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(super) async fn recent_contexts(
        &self,
        user_id: &UserId,
        end: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<RecentContextRow>, RepositoryError> {
        let rows = sqlx::query_as!(
            RecentContextDbRow,
            r#"
            SELECT
                ctx.context_id as "context_id!: ContextId",
                ctx.last_activity as "last_activity!",
                ctx.ai_requests as "ai_requests!",
                NULLIF(last_row.model, '') as "model?",
                NULLIF(last_row.agent_name, '') as "agent_name?"
            FROM (
                SELECT
                    context_id,
                    MAX(last_activity) AS last_activity,
                    SUM(request_count)::bigint AS ai_requests
                FROM report_agent_ai_context_hourly
                WHERE user_id = $1
                  AND bucket < $2
                  AND traffic = 'production'
                GROUP BY context_id
                ORDER BY MAX(last_activity) DESC NULLS LAST
                LIMIT $3
            ) ctx
            LEFT JOIN LATERAL (
                SELECT model, agent_name FROM report_agent_ai_context_hourly
                WHERE context_id = ctx.context_id
                ORDER BY last_activity DESC NULLS LAST
                LIMIT 1
            ) last_row ON TRUE
            ORDER BY ctx.last_activity DESC
            "#,
            user_id as _,
            end,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        let ids: Vec<String> = rows.iter().map(|row| row.context_id.to_string()).collect();
        let names: HashMap<String, Option<String>> = sqlx::query!(
            "SELECT context_id, name FROM report_user_contexts WHERE context_id = ANY($1)",
            &ids
        )
        .fetch_all(&*self.pool)
        .await?
        .into_iter()
        .filter_map(|row| Some((row.context_id?, row.name)))
        .collect();
        Ok(rows
            .into_iter()
            .map(|row| RecentContextRow {
                context_name: names.get(row.context_id.as_str()).cloned().flatten(),
                context_id: row.context_id,
                last_activity: row.last_activity,
                ai_requests: row.ai_requests,
                model: row.model,
                agent_name: row.agent_name.map(AgentName::new),
            })
            .collect())
    }
}
