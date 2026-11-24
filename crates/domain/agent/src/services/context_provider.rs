//! Adapter exposing [`ContextRepository`] through the
//! [`systemprompt_traits::ContextProvider`] trait so other crates can consume
//! it without a direct database dependency.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use async_trait::async_trait;
use systemprompt_identifiers::{ContextId, SessionId, UserId};
use systemprompt_traits::{
    ContextDirectory, ContextMaterializer, ContextProvider, ContextProviderError, ContextWithStats,
    EnsureContextParams, OwnedContextSummary, SessionContextIssuer,
};

use crate::models::context::ContextKind;
use crate::repository::ContextRepository;

#[derive(Debug, Clone)]
pub struct ContextProviderService {
    repo: ContextRepository,
}

impl ContextProviderService {
    #[must_use]
    pub const fn new(repo: ContextRepository) -> Self {
        Self { repo }
    }
}

impl ContextProvider for ContextProviderService {
    async fn list_contexts_with_stats(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<ContextWithStats>, ContextProviderError> {
        let contexts = self
            .repo
            .list_contexts_with_stats(user_id)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))?;

        Ok(contexts
            .into_iter()
            .map(|c| ContextWithStats {
                context_id: c.context_id,
                user_id: c.user_id,
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
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<ContextWithStats, ContextProviderError> {
        let context_with_stats = self
            .repo
            .get_context_with_stats(context_id, user_id)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound { .. } => {
                    ContextProviderError::NotFound(format!("Context {context_id} not found"))
                },
                other => ContextProviderError::Database(other.into()),
            })?;

        Ok(ContextWithStats {
            context_id: context_with_stats.context_id,
            user_id: context_with_stats.user_id,
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
        user_id: &UserId,
        session_id: Option<&SessionId>,
        name: &str,
    ) -> Result<ContextId, ContextProviderError> {
        self.repo
            .create_context(user_id, session_id, name, ContextKind::User)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))
    }

    async fn update_context_name(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), ContextProviderError> {
        self.repo
            .update_context_name(context_id, user_id, name)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound { .. } => {
                    ContextProviderError::NotFound(format!("Context {context_id} not found"))
                },
                other => ContextProviderError::Database(other.into()),
            })
    }

    async fn delete_context(
        &self,
        context_id: &ContextId,
        user_id: &UserId,
    ) -> Result<(), ContextProviderError> {
        self.repo
            .delete_context(context_id, user_id)
            .await
            .map_err(|e| match e {
                systemprompt_traits::RepositoryError::NotFound { .. } => {
                    ContextProviderError::NotFound(format!("Context {context_id} not found"))
                },
                other => ContextProviderError::Database(other.into()),
            })
    }
}

#[async_trait]
impl ContextMaterializer for ContextProviderService {
    async fn ensure_context(
        &self,
        params: EnsureContextParams<'_>,
    ) -> Result<(), ContextProviderError> {
        let kind = params
            .kind
            .parse::<ContextKind>()
            .map_err(|e| ContextProviderError::Internal(e.into()))?;

        if kind == ContextKind::Legacy {
            return self
                .repo
                .ensure_legacy_context(params.user_id)
                .await
                .map_err(|e| ContextProviderError::Database(e.into()));
        }

        self.repo
            .ensure_context(&params, kind)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))
    }
}

#[async_trait]
impl SessionContextIssuer for ContextProviderService {
    async fn session_context(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        name: &str,
    ) -> Result<ContextId, ContextProviderError> {
        self.repo
            .get_or_create_cli_context(user_id, session_id, name)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))
    }
}

#[async_trait]
impl ContextDirectory for ContextProviderService {
    async fn owned_context(
        &self,
        user_id: &UserId,
        context_id: &ContextId,
    ) -> Result<Option<OwnedContextSummary>, ContextProviderError> {
        let name = self
            .repo
            .find_owned_name(context_id, user_id)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))?;
        Ok(name.map(|name| OwnedContextSummary {
            context_id: context_id.clone(),
            name,
        }))
    }

    async fn owned_names(
        &self,
        user_id: &UserId,
        context_ids: &[ContextId],
    ) -> Result<HashMap<ContextId, String>, ContextProviderError> {
        if context_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let names = self
            .repo
            .owned_names(user_id, context_ids)
            .await
            .map_err(|e| ContextProviderError::Database(e.into()))?;
        Ok(names.into_iter().collect())
    }
}
