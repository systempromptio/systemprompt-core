pub mod api;
pub mod cli;
pub mod middleware;
pub mod models;
pub mod orchestration;
pub mod repository;
pub mod services;

pub use systemprompt_models::mcp::{
    Deployment, DeploymentConfig, McpAuthState, McpServerConfig, OAuthRequirement, Settings, ERROR,
    RUNNING, STARTING, STOPPED,
};

pub use services::monitoring::health::HealthStatus;
pub use services::registry::trait_impl::McpDeploymentProviderImpl;
pub use services::registry::McpServerRegistry;
pub use services::tool_provider::McpToolProvider;
pub use services::{EventBus as McpEventBus, McpEvent, McpManager, ServiceManager};

pub use orchestration::{
    McpServerConnectionInfo, McpServerMetadata, McpServiceState, McpToolLoader, ServerStatus,
    ServiceStateManager, SkillLoadingResult,
};

pub use systemprompt_models::mcp::{
    DynMcpDeploymentProvider, DynMcpRegistry, DynMcpToolProvider, McpDeploymentProvider,
    McpProvider, McpRegistry, McpServerState,
};

pub fn mcp_protocol_version() -> String {
    ProtocolVersion::LATEST.to_string()
}

pub mod registry {
    pub use crate::services::registry::RegistryManager;
}

pub use cli::{list_services, show_status, start_services, stop_services};

pub use rmcp::model::ProtocolVersion;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::ServerHandler;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_runtime::AppContext;

use crate::middleware::DatabaseSessionManager;

pub fn create_router<S>(server: S, app_context: &Arc<AppContext>) -> axum::Router
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let config = StreamableHttpServerConfig {
        stateful_mode: true,
        sse_keep_alive: Some(Duration::from_secs(30)),
    };

    let session_manager = DatabaseSessionManager::new(Arc::clone(app_context.db_pool()));

    let service =
        StreamableHttpService::new(move || Ok(server.clone()), session_manager.into(), config);

    axum::Router::new().nest_service("/mcp", service)
}
