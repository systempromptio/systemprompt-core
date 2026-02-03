use std::fmt;
use std::sync::Arc;

use futures::Stream;
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::common::server_side_http::ServerSseMessage;
use rmcp::transport::streamable_http_server::session::local::{
    LocalSessionManager, LocalSessionManagerError,
};
use rmcp::transport::streamable_http_server::session::{SessionId, SessionManager};
use systemprompt_database::DbPool;
use tokio::sync::RwLock;

use crate::repository::McpSessionRepository;

#[derive(Debug)]
pub enum DatabaseSessionManagerError {
    Local(LocalSessionManagerError),
    Database(anyhow::Error),
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
            Self::Database(e) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl From<LocalSessionManagerError> for DatabaseSessionManagerError {
    fn from(e: LocalSessionManagerError) -> Self {
        Self::Local(e)
    }
}

#[derive(Debug)]
pub struct DatabaseSessionManager {
    local_manager: LocalSessionManager,
    repository: Arc<RwLock<Option<McpSessionRepository>>>,
}

impl DatabaseSessionManager {
    pub fn new(db_pool: DbPool) -> Self {
        let repository = McpSessionRepository::new(&db_pool).ok();
        Self {
            local_manager: LocalSessionManager::default(),
            repository: Arc::new(RwLock::new(repository)),
        }
    }

    async fn persist_create(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref() {
            if let Err(e) = repo.create(session_id.as_ref(), None, None).await {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to persist session creation to database"
                );
            }
        }
    }

    async fn persist_close(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref() {
            if let Err(e) = repo.close(session_id.as_ref()).await {
                tracing::warn!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to persist session close to database"
                );
            }
        }
    }

    async fn update_activity(&self, session_id: &SessionId) {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref() {
            if let Err(e) = repo.update_activity(session_id.as_ref()).await {
                tracing::debug!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to update session activity"
                );
            }
        }
    }

    async fn check_db_session(&self, session_id: &SessionId) -> Option<bool> {
        let repo_guard = self.repository.read().await;
        if let Some(repo) = repo_guard.as_ref() {
            match repo.find_active(session_id.as_ref()).await {
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

impl SessionManager for DatabaseSessionManager {
    type Error = DatabaseSessionManagerError;
    type Transport = <LocalSessionManager as SessionManager>::Transport;

    async fn create_session(&self) -> Result<(SessionId, Self::Transport), Self::Error> {
        let (id, transport) = self.local_manager.create_session().await?;
        self.persist_create(&id).await;
        Ok((id, transport))
    }

    async fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<ServerJsonRpcMessage, Self::Error> {
        let result = self.local_manager.initialize_session(id, message).await?;
        self.update_activity(id).await;
        Ok(result)
    }

    async fn has_session(&self, id: &SessionId) -> Result<bool, Self::Error> {
        if self.local_manager.has_session(id).await? {
            return Ok(true);
        }
        Ok(self.check_db_session(id).await.unwrap_or(false))
    }

    async fn close_session(&self, id: &SessionId) -> Result<(), Self::Error> {
        let _ = self.local_manager.close_session(id).await;
        self.persist_close(id).await;
        Ok(())
    }

    async fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error> {
        let stream = self.local_manager.create_stream(id, message).await?;
        self.update_activity(id).await;
        Ok(stream)
    }

    async fn accept_message(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> Result<(), Self::Error> {
        self.local_manager.accept_message(id, message).await?;
        self.update_activity(id).await;
        Ok(())
    }

    async fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error> {
        let stream = self.local_manager.create_standalone_stream(id).await?;
        self.update_activity(id).await;
        Ok(stream)
    }

    async fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error> {
        if self.local_manager.has_session(id).await.unwrap_or(false) {
            return self
                .local_manager
                .resume(id, last_event_id)
                .await
                .map_err(Into::into);
        }

        match self.check_db_session(id).await {
            Some(true) => {
                tracing::info!(
                    session_id = %id,
                    "Session exists in database but not in memory - client needs to reconnect"
                );
                Err(DatabaseSessionManagerError::SessionNeedsReconnect(
                    id.to_string(),
                ))
            },
            Some(false) => {
                tracing::debug!(
                    session_id = %id,
                    "Session not found in database"
                );
                Err(DatabaseSessionManagerError::SessionNotFound(id.to_string()))
            },
            None => self
                .local_manager
                .resume(id, last_event_id)
                .await
                .map_err(Into::into),
        }
    }
}
