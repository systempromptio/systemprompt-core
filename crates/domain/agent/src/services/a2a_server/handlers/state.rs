use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_models::{AgentConfig, AiProvider};
use tokio::sync::RwLock;

use crate::services::a2a_server::auth::AgentOAuthState;
use crate::state::AgentState;

#[derive(Clone)]
pub struct AgentHandlerState {
    pub db_pool: DbPool,
    pub config: Arc<RwLock<AgentConfig>>,
    pub oauth_state: Arc<AgentOAuthState>,
    pub agent_state: Arc<AgentState>,
    pub ai_service: Arc<dyn AiProvider>,
}

impl std::fmt::Debug for AgentHandlerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandlerState")
            .field("db_pool", &"<DbPool>")
            .field("config", &"<Arc<RwLock<AgentConfig>>>")
            .field("oauth_state", &"<Arc<AgentOAuthState>>")
            .field("agent_state", &"<Arc<AgentState>>")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .finish()
    }
}
