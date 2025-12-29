use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_models::{AgentConfig, AiProvider};
use systemprompt_runtime::AppContext;
use tokio::sync::RwLock;

use crate::services::a2a_server::auth::AgentOAuthState;

#[derive(Clone)]
pub struct AgentHandlerState {
    pub db_pool: DbPool,
    pub config: Arc<RwLock<AgentConfig>>,
    pub oauth_state: Arc<AgentOAuthState>,
    pub app_context: Arc<AppContext>,
    pub ai_service: Arc<dyn AiProvider>,
}

impl std::fmt::Debug for AgentHandlerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandlerState")
            .field("db_pool", &"<DbPool>")
            .field("config", &"<Arc<RwLock<AgentConfig>>>")
            .field("oauth_state", &"<Arc<AgentOAuthState>>")
            .field("app_context", &"<Arc<AppContext>>")
            .field("ai_service", &"<Arc<dyn AiProvider>>")
            .finish()
    }
}
