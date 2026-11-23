//! [`McpSessionStore`] over [`McpSessionRepository`]: the session store the
//! `mcp` member provides (sealed) to the toolset host.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_extension::mcp::{McpSessionStore, McpSessionStoreError, McpSessionStoreResult};
use systemprompt_identifiers::{McpServerId, SessionId, UserId};

use super::McpSessionRepository;
use crate::error::McpDomainError;

fn store_error(error: McpDomainError) -> McpSessionStoreError {
    McpSessionStoreError(Box::new(error))
}

#[async_trait]
impl McpSessionStore for McpSessionRepository {
    async fn create(
        &self,
        session: &SessionId,
        server: Option<&McpServerId>,
    ) -> McpSessionStoreResult<()> {
        Self::create(self, session, None, server)
            .await
            .map_err(store_error)
    }

    async fn close(&self, session: &SessionId) -> McpSessionStoreResult<()> {
        Self::close(self, session).await.map_err(store_error)
    }

    async fn update_activity(&self, session: &SessionId) -> McpSessionStoreResult<()> {
        Self::update_activity(self, session)
            .await
            .map_err(store_error)
    }

    async fn is_active(&self, session: &SessionId) -> McpSessionStoreResult<bool> {
        Ok(self
            .find_active(session)
            .await
            .map_err(store_error)?
            .is_some())
    }

    async fn store_initialize_params(
        &self,
        session: &SessionId,
        // JSON: MCP `initialize` params, an external protocol object stored verbatim.
        params: &serde_json::Value,
    ) -> McpSessionStoreResult<()> {
        Self::store_initialize_params(self, session, params)
            .await
            .map_err(store_error)
    }

    async fn find_restorable_initialize_params(
        &self,
        session: &SessionId,
        server: &McpServerId,
        user: Option<&UserId>,
        // JSON: MCP `initialize` params, an external protocol object stored verbatim.
    ) -> McpSessionStoreResult<Option<serde_json::Value>> {
        Self::find_restorable_initialize_params(self, session, server, user)
            .await
            .map_err(store_error)
    }

    async fn clear_initialize_params(&self, session: &SessionId) -> McpSessionStoreResult<()> {
        Self::clear_initialize_params(self, session)
            .await
            .map_err(store_error)
    }
}
