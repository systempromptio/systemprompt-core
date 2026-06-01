//! Database-backed MCP session manager.
//!
//! [`DatabaseSessionHandler`] implements the rmcp `SessionManager` trait (see
//! [`session_manager_impl`]), wrapping rmcp's in-memory `LocalSessionManager`
//! while mirroring session lifecycle (create, activity, close) into the
//! `mcp_sessions` table for cross-restart visibility.
//! [`DatabaseSessionManagerError`] models the local, database, and
//! reconnect-signalling failure cases; database persistence is best-effort and
//! never fails an in-memory operation.

mod session_manager_impl;

use std::fmt;
use std::sync::Arc;

use rmcp::transport::streamable_http_server::session::SessionId;
use rmcp::transport::streamable_http_server::session::local::{
    LocalSessionManager, LocalSessionManagerError,
};
use systemprompt_database::DbPool;
use tokio::sync::RwLock;

use crate::repository::McpSessionRepository;

#[derive(Debug)]
pub enum DatabaseSessionManagerError {
    Local(LocalSessionManagerError),
    Database(crate::error::McpDomainError),
    SessionNotFound(String),
    SessionExpired(String),
    SessionNeedsReconnect(String),
}

impl fmt::Display for DatabaseSessionManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(e) => write!(f, "Local session error: {e}"),
            Self::Database(e) => write!(f, "Database error: {e}"),
            Self::SessionNotFound(id) => write!(f, "Session not found: {id}"),
            Self::SessionExpired(id) => write!(f, "Session expired: {id}"),
            Self::SessionNeedsReconnect(id) => write!(f, "Session needs reconnect: {id}"),
        }
    }
}

impl std::error::Error for DatabaseSessionManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Local(e) => Some(e),
            Self::Database(e) => Some(e),
            _ => None,
        }
    }
}

impl From<LocalSessionManagerError> for DatabaseSessionManagerError {
    fn from(e: LocalSessionManagerError) -> Self {
        Self::Local(e)
    }
}

pub struct DatabaseSessionHandler {
    local_manager: LocalSessionManager,
    repository: Arc<RwLock<Option<McpSessionRepository>>>,
}

impl fmt::Debug for DatabaseSessionHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseSessionHandler")
            .field("local_manager", &self.local_manager)
            .field("repository", &self.repository)
            .finish()
    }
}

impl DatabaseSessionHandler {
    pub fn new(db_pool: &DbPool) -> Self {
        Self::with_timeouts(db_pool, crate::SessionTimeouts::default())
    }

    pub fn with_timeouts(db_pool: &DbPool, timeouts: crate::SessionTimeouts) -> Self {
        let mut local_manager = LocalSessionManager::default();
        let cfg = &mut local_manager.session_config;
        cfg.init_timeout = timeouts.init.or(cfg.init_timeout);
        cfg.keep_alive = timeouts.keep_alive.or(cfg.keep_alive);
        Self {
            local_manager,
            repository: Arc::new(RwLock::new(McpSessionRepository::new(db_pool).ok())),
        }
    }

    async fn persist_create(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref()
            && let Err(e) = repo
                .create(
                    &systemprompt_identifiers::SessionId::new(session_id.as_ref()),
                    None,
                    None,
                )
                .await
        {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to persist session creation to database"
            );
        }
    }

    async fn persist_close(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref()
            && let Err(e) = repo
                .close(&systemprompt_identifiers::SessionId::new(
                    session_id.as_ref(),
                ))
                .await
        {
            tracing::warn!(
                session_id = %session_id,
                error = %e,
                "Failed to persist session close to database"
            );
        }
    }

    pub(crate) async fn update_activity(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref()
            && let Err(e) = repo
                .update_activity(&systemprompt_identifiers::SessionId::new(
                    session_id.as_ref(),
                ))
                .await
        {
            tracing::debug!(
                session_id = %session_id,
                error = %e,
                "Failed to update session activity"
            );
        }
    }

    async fn check_db_session(&self, session_id: &SessionId) -> Option<bool> {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref() {
            match repo
                .find_active(&systemprompt_identifiers::SessionId::new(
                    session_id.as_ref(),
                ))
                .await
            {
                Ok(Some(_)) => Some(true),
                Ok(None) => Some(false),
                Err(e) => {
                    tracing::warn!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to check session in database"
                    );
                    None
                },
            }
        } else {
            None
        }
    }
}
