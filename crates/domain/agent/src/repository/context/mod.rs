pub mod message;

use crate::models::context::{ContextStateEvent, UserContext, UserContextWithStats};
use crate::repository::task::constructor::TaskConstructor;
use crate::repository::Repository;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, TaskId, UserId};
use systemprompt_traits::RepositoryError;

#[derive(Debug, Clone)]
pub struct ContextRepository {
    db_pool: DbPool,
}

impl ContextRepository {
    #[must_use]
    pub const fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    fn get_pg_pool(&self) -> Result<Arc<PgPool>, RepositoryError> {
        self.db_pool
            .as_ref()
            .get_postgres_pool()
            .ok_or_else(|| RepositoryError::Database("PostgreSQL pool not available".to_string()))
    }

    pub async fn create_context(
        &self,
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> Result<ContextId, RepositoryError> {
        let context_id = ContextId::generate();
        let pool = self.get_pg_pool()?;
        let now = Utc::now();
        let session_id_str = session_id.map(SessionId::as_str);

        sqlx::query!(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name, created_at, \
             updated_at)
             VALUES ($1, $2, $3, $4, $5, $5)",
            context_id.as_str(),
            user_id.as_str(),
            session_id_str,
            name,
            now
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(context_id)
    }

    pub async fn validate_context_ownership(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;

        let result = sqlx::query_scalar!(
            "SELECT context_id FROM user_contexts WHERE context_id = $1 AND user_id = $2",
            context_id.as_str(),
            user_id.as_str()
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        match result {
            Some(_) => Ok(()),
            None => Err(RepositoryError::NotFound(format!(
                "Context {} not found or user {} does not have access",
                context_id, user_id
            ))),
        }
    }

    pub async fn get_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<UserContext, RepositoryError> {
        let pool = self.get_pg_pool()?;

        let row = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                user_id as "user_id!",
                name as "name!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_contexts WHERE context_id = $1 AND user_id = $2"#,
            context_id.as_str(),
            user_id.as_str()
        )
        .fetch_one(pool.as_ref())
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(format!(
                "Context {} not found for user {}",
                context_id, user_id
            )),
            _ => RepositoryError::Database(e.to_string()),
        })?;

        Ok(UserContext {
            context_id: row.context_id.into(),
            user_id: row.user_id.into(),
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub async fn list_contexts_basic(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserContext>, RepositoryError> {
        let pool = self.get_pg_pool()?;

        let rows = sqlx::query!(
            r#"SELECT
                context_id as "context_id!",
                user_id as "user_id!",
                name as "name!",
                created_at as "created_at!",
                updated_at as "updated_at!"
            FROM user_contexts WHERE user_id = $1 ORDER BY updated_at DESC"#,
            user_id.as_str()
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| UserContext {
                context_id: r.context_id.into(),
                user_id: r.user_id.into(),
                name: r.name,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    pub async fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<UserContextWithStats>, RepositoryError> {
        let pool = self.get_pg_pool()?;

        let rows = sqlx::query!(
            r#"SELECT
                c.context_id as "context_id!",
                c.user_id as "user_id!",
                c.name as "name!",
                c.created_at as "created_at!",
                c.updated_at as "updated_at!",
                COALESCE(COUNT(DISTINCT t.task_id), 0)::bigint as "task_count!",
                COALESCE(COUNT(DISTINCT m.id), 0)::bigint as "message_count!",
                MAX(m.created_at) as last_message_at
            FROM user_contexts c
            LEFT JOIN agent_tasks t ON t.context_id = c.context_id
            LEFT JOIN task_messages m ON m.task_id = t.task_id
            WHERE c.user_id = $1
            GROUP BY c.context_id
            ORDER BY c.updated_at DESC"#,
            user_id.as_str()
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| UserContextWithStats {
                context_id: r.context_id.into(),
                user_id: r.user_id.into(),
                name: r.name,
                created_at: r.created_at,
                updated_at: r.updated_at,
                task_count: r.task_count,
                message_count: r.message_count,
                last_message_at: r.last_message_at,
            })
            .collect())
    }

    pub async fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;
        let now = Utc::now();

        let result = sqlx::query!(
            "UPDATE user_contexts SET name = $1, updated_at = $2
             WHERE context_id = $3 AND user_id = $4",
            name,
            now,
            context_id.as_str(),
            user_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Context {} not found for user {}",
                context_id, user_id
            )));
        }

        Ok(())
    }

    pub async fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), RepositoryError> {
        let pool = self.get_pg_pool()?;

        let result = sqlx::query!(
            "DELETE FROM user_contexts WHERE context_id = $1 AND user_id = $2",
            context_id.as_str(),
            user_id.as_str()
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound(format!(
                "Context {} not found for user {}",
                context_id, user_id
            )));
        }

        Ok(())
    }

    pub async fn get_context_events_since(
        &self,
        context_id: &ContextId,
        last_seen: DateTime<Utc>,
    ) -> Result<Vec<ContextStateEvent>, RepositoryError> {
        let mut events = Vec::new();
        let pool = self.get_pg_pool()?;

        let task_ids: Vec<String> = sqlx::query_scalar!(
            r#"SELECT t.task_id as "task_id!" FROM agent_tasks t
             WHERE t.context_id = $1 AND t.updated_at > $2
             ORDER BY t.updated_at ASC"#,
            context_id.as_str(),
            last_seen
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        if !task_ids.is_empty() {
            let constructor = TaskConstructor::new(self.db_pool.clone());
            let task_ids_typed: Vec<TaskId> = task_ids.iter().map(|id| TaskId::new(id)).collect();
            let tasks = constructor.construct_tasks_batch(&task_ids_typed).await?;

            for task in tasks {
                events.push(ContextStateEvent::TaskStatusChanged {
                    task,
                    context_id: context_id.to_string(),
                    timestamp: Utc::now(),
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
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;

        for row in context_updates {
            events.push(ContextStateEvent::ContextUpdated {
                context_id: row.context_id,
                name: row.name,
                timestamp: row.updated_at,
            });
        }

        events.sort_by(|a, b| a.timestamp().cmp(&b.timestamp()));

        Ok(events)
    }
}

impl Repository for ContextRepository {
    fn pool(&self) -> &DbPool {
        &self.db_pool
    }
}

impl systemprompt_traits::Repository for ContextRepository {
    type Pool = DbPool;
    type Error = RepositoryError;

    fn pool(&self) -> &Self::Pool {
        &self.db_pool
    }
}
