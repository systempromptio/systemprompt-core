//! [`SessionManager`] implementation for [`DatabaseSessionHandler`], delegating
//! to the in-memory `LocalSessionManager` and mirroring lifecycle to the
//! database.

use futures::{Stream, StreamExt};
use rmcp::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};
use rmcp::transport::common::server_side_http::ServerSseMessage;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::session::{SessionId, SessionManager};

use super::{DatabaseSessionHandler, DatabaseSessionManagerError};

impl SessionManager for DatabaseSessionHandler {
    type Error = DatabaseSessionManagerError;
    type Transport = <LocalSessionManager as SessionManager>::Transport;

    async fn create_session(&self) -> Result<(SessionId, Self::Transport), Self::Error> {
        let (id, transport) = self.local_manager.create_session().await?;
        tracing::info!(session_id = %id, "MCP session created");
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
        if self.local_manager.has_session(id).await.unwrap_or(false) {
            return Ok(true);
        }
        if self.check_db_session(id).await == Some(true) {
            tracing::info!(
                session_id = %id,
                "Session in DB but not memory — session not available (client should re-initialize)"
            );
        }
        Ok(false)
    }

    async fn close_session(&self, id: &SessionId) -> Result<(), Self::Error> {
        tracing::info!(session_id = %id, "MCP session closing");
        if let Err(e) = self.local_manager.close_session(id).await {
            tracing::warn!(session_id = %id, error = %e, "Failed to close local session");
        }
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
        if !self.local_manager.has_session(id).await.unwrap_or(false) {
            if self.check_db_session(id).await == Some(true) {
                tracing::info!(
                    session_id = %id,
                    "Session in DB but not memory (server restart?) — signaling reconnect"
                );
                self.persist_close(id).await;
                return Err(DatabaseSessionManagerError::SessionNeedsReconnect(
                    id.to_string(),
                ));
            }
            tracing::warn!(
                session_id = %id,
                "Resume called but session not found anywhere"
            );
            return Err(DatabaseSessionManagerError::SessionNotFound(id.to_string()));
        }

        match self.local_manager.resume(id, last_event_id).await {
            Ok(stream) => {
                tracing::info!(
                    session_id = %id,
                    "Session resumed successfully"
                );
                self.update_activity(id).await;
                Ok(stream.left_stream())
            },
            Err(e) => {
                tracing::info!(
                    session_id = %id,
                    error = %e,
                    "Resume failed, attempting recovery via new standalone stream"
                );
                match self.local_manager.create_standalone_stream(id).await {
                    Ok(stream) => {
                        tracing::info!(
                            session_id = %id,
                            "Session recovered with new standalone stream"
                        );
                        self.update_activity(id).await;
                        Ok(stream.right_stream())
                    },
                    Err(e2) => {
                        tracing::warn!(
                            session_id = %id,
                            error = %e2,
                            "Session worker is dead, cleaning up"
                        );
                        if let Err(e) = self.local_manager.close_session(id).await {
                            tracing::warn!(session_id = %id, error = %e, "Failed to close local session during recovery");
                        }
                        self.persist_close(id).await;
                        Err(DatabaseSessionManagerError::SessionNotFound(id.to_string()))
                    },
                }
            },
        }
    }
}
