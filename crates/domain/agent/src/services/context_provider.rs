//! Implementation of ContextProvider trait for the agent module.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_traits::{ContextProvider, ContextProviderError, ContextWithStats};

use crate::repository::ContextRepository;

#[derive(Debug, Clone)]
pub struct ContextProviderService {
    repo: ContextRepository,
}

impl ContextProviderService {
    pub fn new(db_pool: &DbPool) -> anyhow::Result<Self> {
        Ok(Self {
            repo: ContextRepository::new(db_pool)?,
        })
    }
}

#[async_trait]
impl ContextProvider for ContextProviderService {
    async fn list_contexts_with_stats(
        &self,
        user_id: &str,
    ) -> Result<Vec<ContextWithStats>, ContextProviderError> {
        let user_id = UserId::new(user_id);
        let contexts = self
            .repo
            .list_contexts_with_stats(&user_id)
            .await
            .map_err(|e| ContextProviderError::Database(e.to_string()))?;

        Ok(contexts
            .into_iter()
            .map(|c| ContextWithStats {
                context_id: c.context_id.to_string(),
                user_id: c.user_id.to_string(),
                name: c.name,
                created_at: c.created_at,
                updated_at: c.updated_at,
                task_count: c.task_count,
                message_count: c.message_count,
                last_message_at: c.last_message_at,
            })
            .collect())
    }

    async fn get_context(
        &self,
        context_id: &str,
        user_id: &str,
    ) -> Result<ContextWithStats, ContextProviderError> {
        let context_id = ContextId::new(context_id);
        let user_id = UserId::new(user_id);

        let context = self
            .repo
            .get_context(&context_id, &user_id)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound(msg) => {
                    ContextProviderError::NotFound(msg)
                },
                other => ContextProviderError::Database(other.to_string()),
            })?;

        let all_contexts = self
            .repo
            .list_contexts_with_stats(&user_id)
            .await
            .map_err(|e| ContextProviderError::Database(e.to_string()))?;

        let context_with_stats = all_contexts
            .into_iter()
            .find(|c| c.context_id == context.context_id)
            .ok_or_else(|| {
                ContextProviderError::NotFound(format!("Context {} not found", context_id))
            })?;

        Ok(ContextWithStats {
            context_id: context_with_stats.context_id.to_string(),
            user_id: context_with_stats.user_id.to_string(),
            name: context_with_stats.name,
            created_at: context_with_stats.created_at,
            updated_at: context_with_stats.updated_at,
            task_count: context_with_stats.task_count,
            message_count: context_with_stats.message_count,
            last_message_at: context_with_stats.last_message_at,
        })
    }

    async fn create_context(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        name: &str,
    ) -> Result<String, ContextProviderError> {
        let user_id = UserId::new(user_id);
        let session_id = session_id.map(SessionId::new);

        let context_id = self
            .repo
            .create_context(&user_id, session_id.as_ref(), name)
            .await
            .map_err(|e| ContextProviderError::Database(e.to_string()))?;

        Ok(context_id.to_string())
    }

    async fn update_context_name(
        &self,
        context_id: &str,
        user_id: &str,
        name: &str,
    ) -> Result<(), ContextProviderError> {
        let context_id = ContextId::new(context_id);
        let user_id = UserId::new(user_id);

        self.repo
            .update_context_name(&context_id, &user_id, name)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound(msg) => {
                    ContextProviderError::NotFound(msg)
                },
                other => ContextProviderError::Database(other.to_string()),
            })
    }

    async fn delete_context(
        &self,
        context_id: &str,
        user_id: &str,
    ) -> Result<(), ContextProviderError> {
        let context_id = ContextId::new(context_id);
        let user_id = UserId::new(user_id);

        self.repo
            .delete_context(&context_id, &user_id)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound(msg) => {
                    ContextProviderError::NotFound(msg)
                },
                other => ContextProviderError::Database(other.to_string()),
            })
    }
}
