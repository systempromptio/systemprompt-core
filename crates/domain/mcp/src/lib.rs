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

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use rmcp::ServerHandler;
pub use rmcp::model::ProtocolVersion;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::StreamableHttpServerConfig;
use std::time::Duration;
use systemprompt_database::DbPool;
use tokio_util::sync::CancellationToken;

use crate::middleware::DatabaseSessionManager;

pub use state::McpState;

async fn mcp_request_logger(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let session_id = req
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let has_auth = req.headers().get("authorization").is_some();
    let proxy_verified = req
        .headers()
        .get("x-proxy-verified")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "true");
    let accept = req
        .headers()
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    tracing::info!(
        %method,
        %uri,
        session_id = ?session_id,
        has_auth,
        proxy_verified,
        accept = ?accept,
        "MCP request received"
    );

    let response = next.run(req).await;

    let status = response.status();
    if !status.is_success() {
        tracing::error!(
            %method,
            %uri,
            session_id = ?session_id,
            status = %status,
            "MCP request failed at transport level"
        );
    }

    response
}

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
        .layer(axum::middleware::from_fn(mcp_request_logger))
        .layer(axum::middleware::map_response(
            |mut response: http::Response<_>| async move {
                response
                    .headers_mut()
                    .insert("x-accel-buffering", http::HeaderValue::from_static("no"));
                response
            },
        ))
}
