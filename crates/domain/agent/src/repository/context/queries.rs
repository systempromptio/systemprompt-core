//! Context read queries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use super::ContextRepository;
use crate::models::context::{ContextKind, ContextStateEvent, UserContext, UserContextWithStats};
use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

fn stored_context_id(raw: String) -> Result<ContextId, RepositoryError> {
    ContextId::try_new(raw).map_err(|e| RepositoryError::decode("context_id", e))
}

/// Counters still NULL belong to a context queued for its first count
/// (migration 015); they read as zero until the drainer fills them.
struct ContextStatsRow {
    context_id: ContextId,
    user_id: UserId,
    name: String,
    kind: ContextKind,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    task_count: i64,
    message_count: i64,
    last_message_at: Option<DateTime<Utc>>,
}

impl ContextStatsRow {
    fn into_stats(self) -> UserContextWithStats {
        UserContextWithStats {
            context_id: self.context_id,
            user_id: self.user_id,
            name: self.name,
            kind: self.kind,
            created_at: self.created_at,
            updated_at: self.updated_at,
            task_count: self.task_count,
            message_count: self.message_count,
            last_message_at: self.last_message_at,
        }
    }
}

impl ContextRepository {
    pub async fn find_user_id_for_context(
        &self,
        context_id: &ContextId,
    ) -> Result<Option<UserId>, RepositoryError> {
        let row = sqlx::query_scalar!(
            r#"SELECT user_id FROM user_contexts WHERE context_id = $1"#,
            context_id.as_str(),
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.map(UserId::new))
    }

    pub async fn get_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<UserContext, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                user_id as "user_id!",
                name as "name!",
                kind as "kind!: ContextKind",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_contexts WHERE context_id = $1 AND user_id = $2"#,
            context_id.as_str(),
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => {
                RepositoryError::not_found("context", format!("{context_id} for user {user_id}"))
            },
            _ => RepositoryError::database(e),
        })?;

        Ok(UserContext {
            context_id: stored_context_id(row.context_id)?,
            user_id: UserId::new(row.user_id),
            name: row.name,
            kind: row.kind,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn list_contexts_basic(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserContext>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                user_id as "user_id!",
                name as "name!",
                kind as "kind!: ContextKind",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_contexts WHERE user_id = $1 ORDER BY updated_at DESC"#,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        rows.into_iter()
            .map(|r| {
                Ok(UserContext {
                    context_id: stored_context_id(r.context_id)?,
                    user_id: UserId::new(r.user_id),
                    name: r.name,
                    kind: r.kind,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
            })
            .collect()
    }

    pub async fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserContextWithStats>, RepositoryError> {
        let rows = sqlx::query_as!(
            ContextStatsRow,
            r#"SELECT context_id as "context_id: ContextId", user_id as "user_id: UserId", name,
                kind as "kind: ContextKind", created_at,
                updated_at, COALESCE(task_count, 0) as "task_count!",
                COALESCE(message_count, 0) as "message_count!", last_message_at
            FROM user_contexts
            WHERE user_id = $1
            ORDER BY updated_at DESC"#,
            user_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(ContextStatsRow::into_stats).collect())
    }

    pub async fn get_context_with_stats(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<UserContextWithStats, RepositoryError> {
        let row = sqlx::query_as!(
            ContextStatsRow,
            r#"SELECT context_id as "context_id: ContextId", user_id as "user_id: UserId", name,
                kind as "kind: ContextKind", created_at,
                updated_at, COALESCE(task_count, 0) as "task_count!",
                COALESCE(message_count, 0) as "message_count!", last_message_at
            FROM user_contexts
            WHERE context_id = $1 AND user_id = $2"#,
            context_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?
        .ok_or_else(|| {
            RepositoryError::not_found("context", format!("{context_id} for user {user_id}"))
        })?;

        Ok(row.into_stats())
    }

    pub async fn find_by_session_id(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<UserContext>, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                user_id as "user_id!",
                name as "name!",
                kind as "kind!: ContextKind",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_contexts WHERE session_id = $1
            ORDER BY created_at DESC LIMIT 1"#,
            session_id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        row.map(|r| {
            Ok(UserContext {
                context_id: stored_context_id(r.context_id)?,
                user_id: UserId::new(r.user_id),
                name: r.name,
                kind: r.kind,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
        })
        .transpose()
    }

    pub async fn get_context_events_since(
        &self,
        context_id: &ContextId,
        last_seen: DateTime<Utc>,
    ) -> Result<Vec<ContextStateEvent>, RepositoryError> {
        let mut events = Vec::new();

        let task_ids: Vec<String> = sqlx::query_scalar!(
            r#"SELECT t.task_id as "task_id!" FROM agent_tasks t
             WHERE t.context_id = $1 AND t.updated_at > $2
             ORDER BY t.updated_at ASC"#,
            context_id.as_str(),
            last_seen
        )
        .fetch_all(&*self.pool)
        .await?;

        if !task_ids.is_empty() {
            let task_ids_typed: Vec<TaskId> = task_ids.iter().map(TaskId::new).collect();
            let tasks = self.tasks.construct_tasks_batch(&task_ids_typed).await?;

            for task in tasks {
                let timestamp = task.last_modified.unwrap_or(last_seen);
                events.push(ContextStateEvent::TaskStatusChanged {
                    task,
                    context_id: context_id.clone(),
                    timestamp,
                });
            }
        }

        let context_updates = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                name as "name!",
                updated_at as "updated_at!"
            FROM user_contexts
            WHERE context_id = $1 AND updated_at > $2
            ORDER BY updated_at ASC"#,
            context_id.as_str(),
            last_seen
        )
        .fetch_all(&*self.pool)
        .await?;

        for row in context_updates {
            events.push(ContextStateEvent::ContextUpdated {
                context_id: stored_context_id(row.context_id)?,
                name: row.name,
                timestamp: row.updated_at,
            });
        }

        events.sort_by_key(ContextStateEvent::timestamp);

        Ok(events)
    }
}
