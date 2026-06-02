//! Postgres-backed [`SessionStore`] for cross-restart MCP session recovery.
//!
//! rmcp's `StreamableHttpService` calls [`SessionStore::store`] after a
//! successful `initialize`, [`SessionStore::load`] when a request arrives for a
//! session that is no longer in memory, and [`SessionStore::delete`] on
//! teardown. Persisting the original `initialize` params in `mcp_sessions`
//! lets the service transparently re-create a session whose in-memory worker
//! was lost to a server restart or eviction — instead of returning
//! `404 Session not found` and provoking a client reconnect storm. Persistence
//! is best-effort: a missing repository or store error degrades to "no stored
//! state", which simply falls back to the 404/re-initialize path.

use async_trait::async_trait;
use rmcp::model::InitializeRequestParams;
use rmcp::transport::streamable_http_server::session::store::{
    SessionState, SessionStore, SessionStoreError,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;

use crate::repository::McpSessionRepository;

#[derive(Debug)]
pub struct PostgresSessionStore {
    repository: Option<McpSessionRepository>,
}

impl PostgresSessionStore {
    pub fn new(db_pool: &DbPool) -> Self {
        let repository = match McpSessionRepository::new(db_pool) {
            Ok(repository) => Some(repository),
            Err(error) => {
                tracing::warn!(%error, "MCP session store disabled: repository unavailable");
                None
            },
        };
        Self { repository }
    }
}

#[async_trait]
impl SessionStore for PostgresSessionStore {
    async fn load(&self, session_id: &str) -> Result<Option<SessionState>, SessionStoreError> {
        let Some(repo) = self.repository.as_ref() else {
            return Ok(None);
        };
        let Some(value) = repo
            .find_initialize_params(&SessionId::new(session_id))
            .await
            .map_err(boxed)?
        else {
            return Ok(None);
        };
        // JSON: `InitializeRequestParams` is an external rmcp protocol type
        // persisted verbatim as JSONB; deserialize it back into the typed form.
        let params: InitializeRequestParams = serde_json::from_value(value).map_err(boxed)?;
        Ok(Some(SessionState::new(params)))
    }

    async fn store(&self, session_id: &str, state: &SessionState) -> Result<(), SessionStoreError> {
        let Some(repo) = self.repository.as_ref() else {
            return Ok(());
        };
        // JSON: protocol boundary — store the rmcp init params as JSONB.
        let value = serde_json::to_value(&state.initialize_params).map_err(boxed)?;
        repo.store_initialize_params(&SessionId::new(session_id), &value)
            .await
            .map_err(boxed)
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let Some(repo) = self.repository.as_ref() else {
            return Ok(());
        };
        repo.clear_initialize_params(&SessionId::new(session_id))
            .await
            .map_err(boxed)
    }
}

fn boxed<E: std::error::Error + Send + Sync + 'static>(error: E) -> SessionStoreError {
    Box::new(error)
}
