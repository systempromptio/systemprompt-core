//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::{AgentConfig, AiProvider};
use tokio::sync::{RwLock, Semaphore};

use crate::services::a2a_server::auth::AgentOAuthState;
use crate::state::AgentState;

#[derive(Clone)]
pub struct AgentHandlerState {
    pub db_pool: DbPool,
    pub config: Arc<RwLock<AgentConfig>>,
    pub oauth_state: Arc<AgentOAuthState>,
    pub agent_state: Arc<AgentState>,
    pub ai_service: Arc<dyn AiProvider>,
    /// Global cap on concurrently active A2A SSE streams. A permit is held
    /// for the whole lifetime of each spawned stream task; exhaustion bounds
    /// process memory under load rather than spawning tasks without limit.
    pub stream_semaphore: Arc<Semaphore>,
}

impl std::fmt::Debug for AgentHandlerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandlerState")
            .field("db_pool", &"<DbPool>")
            .field("config", &"<Arc<RwLock<AgentConfig>>>")
            .field("oauth_state", &"<Arc<AgentOAuthState>>")
            .field("agent_state", &"<Arc<AgentState>>")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .field(
                "stream_semaphore",
                &self.stream_semaphore.available_permits(),
            )
            .finish()
    }
}
