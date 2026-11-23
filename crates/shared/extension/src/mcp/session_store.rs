//! The MCP session store the toolset host's streamable-HTTP transport reads.
//!
//! [`McpSessionStore`] persists a session's lifecycle (create, activity,
//! close) and the `initialize` params that let a session be restored after a
//! restart. The `mcp` member implements it over `mcp_sessions` and provides
//! it **sealed** (`DynMcpSessionStore`): a restore lookup takes the server and
//! user to match as arguments, so a holder could read or resume any caller's
//! session; only the kernel (the toolset host) reads it, and it passes the
//! principal the hop layer verified for the current request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{McpServerId, SessionId, UserId};
use systemprompt_traits::BoxedSource;

#[derive(Debug, thiserror::Error)]
#[error("the MCP session store failed")]
pub struct McpSessionStoreError(#[source] pub BoxedSource);

pub type McpSessionStoreResult<T> = Result<T, McpSessionStoreError>;

/// Persistence of MCP transport sessions for the toolset host.
///
/// `#[async_trait]` because the store is held as `Arc<dyn McpSessionStore>`.
#[async_trait]
pub trait McpSessionStore: Send + Sync + std::fmt::Debug {
    async fn create(
        &self,
        session: &SessionId,
        server: Option<&McpServerId>,
    ) -> McpSessionStoreResult<()>;

    async fn close(&self, session: &SessionId) -> McpSessionStoreResult<()>;

    async fn update_activity(&self, session: &SessionId) -> McpSessionStoreResult<()>;

    async fn is_active(&self, session: &SessionId) -> McpSessionStoreResult<bool>;

    // JSON: MCP `initialize` params, an external protocol object stored verbatim.
    async fn store_initialize_params(
        &self,
        session: &SessionId,
        params: &serde_json::Value,
    ) -> McpSessionStoreResult<()>;

    // JSON: MCP `initialize` params, an external protocol object stored verbatim.
    async fn find_restorable_initialize_params(
        &self,
        session: &SessionId,
        server: &McpServerId,
        user: Option<&UserId>,
    ) -> McpSessionStoreResult<Option<serde_json::Value>>;

    async fn clear_initialize_params(&self, session: &SessionId) -> McpSessionStoreResult<()>;
}

pub type DynMcpSessionStore = Arc<dyn McpSessionStore>;
