pub mod capabilities;
pub mod cli;
pub mod error;
pub mod extension;
pub mod jobs;
pub mod middleware;
pub mod models;
pub mod orchestration;
pub mod progress;
pub mod repository;
pub mod resources;
pub mod response;
pub mod schema;
pub mod services;
pub mod tool;

pub use extension::McpExtension;

pub use error::McpError as McpDomainError;
pub use rmcp::ErrorData as McpError;
pub type McpResult<T> = Result<T, McpError>;

pub use capabilities::{
    WEBSITE_URL, build_experimental_capabilities, default_tool_visibility, mcp_apps_ui_extension,
    model_only_visibility, tool_ui_meta, visibility_to_json,
};
pub use progress::{ProgressCallback, create_progress_callback};
pub use repository::{CreateMcpArtifact, McpArtifactRecord, McpArtifactRepository};
pub use resources::{
    ArtifactViewerConfig, build_artifact_viewer_resource, default_server_icons,
    read_artifact_viewer_resource,
};
pub use response::McpResponseBuilder;
pub use schema::McpOutputSchema;
pub use tool::{McpToolExecutor, McpToolHandler};

pub use systemprompt_models::mcp::{
    Deployment, DeploymentConfig, ERROR, McpAuthState, McpServerConfig, OAuthRequirement, RUNNING,
    STARTING, STOPPED, Settings,
};

pub use services::monitoring::health::HealthStatus;
pub use services::registry::McpServerRegistry;
pub use services::registry::trait_impl::McpDeploymentProviderImpl;
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

pub mod state;

use rmcp::ServerHandler;
pub use rmcp::model::ProtocolVersion;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use std::time::Duration;
use systemprompt_database::DbPool;
use tokio_util::sync::CancellationToken;

use crate::middleware::DatabaseSessionManager;

pub use state::McpState;

pub fn create_router<S>(server: S, db_pool: &DbPool) -> axum::Router
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let config = StreamableHttpServerConfig {
        stateful_mode: true,
        sse_keep_alive: Some(Duration::from_secs(15)),
        sse_retry: Some(Duration::from_secs(3)),
        cancellation_token: CancellationToken::new(),
        json_response: false,
    };

    let session_manager = DatabaseSessionManager::new(db_pool);

    let service =
        StreamableHttpService::new(move || Ok(server.clone()), session_manager.into(), config);

    axum::Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::map_response(
            |mut response: http::Response<_>| async move {
                response
                    .headers_mut()
                    .insert("x-accel-buffering", http::HeaderValue::from_static("no"));
                response
            },
        ))
}
