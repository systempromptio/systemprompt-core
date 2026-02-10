use crate::services::proxy::ProxyEngine;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::McpServerRegistry;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{ApiError, Config};
use systemprompt_oauth::{GrantType, PkceMethod, ResponseType, TokenAuthMethod};
use systemprompt_runtime::{AppContext, ServiceCategory};
use systemprompt_traits::McpRegistryProvider;

#[derive(Debug, Serialize)]
pub struct ToolExecutionResponse {
    pub id: String,
    pub tool_name: String,
    pub server_name: String,
    pub server_endpoint: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct McpState {
    pub ctx: AppContext,
    pub repo: Arc<ToolUsageRepository>,
}

pub async fn handle_get_execution(
    Path(execution_id): Path<String>,
    State(state): State<McpState>,
) -> impl IntoResponse {
    tracing::info!(execution_id = %execution_id, "Fetching execution");

    let execution_id_typed = McpExecutionId::new(&execution_id);
    match state.repo.find_by_id(&execution_id_typed).await {
        Ok(Some(execution)) => {
            let server_endpoint = ApiPaths::mcp_server_endpoint(&execution.server_name);

            let input = match serde_json::from_str(&execution.input) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(execution_id = %execution_id, error = %e, "Invalid input JSON");
                    return ApiError::internal_error(format!("Invalid input JSON: {e}"))
                        .into_response();
                },
            };

            let response = ToolExecutionResponse {
                id: execution.mcp_execution_id.to_string(),
                tool_name: execution.tool_name,
                server_name: execution.server_name.clone(),
                server_endpoint,
                input,
                output: execution.output.as_deref().and_then(|s| {
                    serde_json::from_str(s)
                        .map_err(|e| {
                            tracing::warn!(
                                execution_id = %execution_id,
                                error = %e,
                                "Failed to parse execution output JSON"
                            );
                            e
                        })
                        .ok()
                }),
                status: execution.status,
            };

            tracing::info!(execution_id = %execution_id, "Execution found");
            Json(response).into_response()
        },
        Ok(None) => {
            ApiError::not_found(format!("Execution not found: {execution_id}")).into_response()
        },
        Err(e) => {
            tracing::error!(execution_id = %execution_id, error = %e, "Failed to get execution");
            ApiError::internal_error(format!("Failed to get execution: {e}")).into_response()
        },
    }
}

#[derive(Debug, Serialize)]
pub struct McpProtectedResourceMetadata {
    pub resource: String,
    pub authorization_servers: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub bearer_methods_supported: Vec<String>,
    pub resource_documentation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct McpAuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub registration_endpoint: Option<String>,
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
}

pub async fn handle_mcp_protected_resource(Path(service_name): Path<String>) -> impl IntoResponse {
    let base_url = match Config::get() {
        Ok(c) => c.api_external_url.clone(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Configuration unavailable"})),
            )
                .into_response();
        },
    };

    let scopes = match get_mcp_server_scopes(&service_name).await {
        Some(s) => s,
        None => vec!["user".to_string()],
    };

    let resource_url = format!("{}/api/v1/mcp/{}/mcp", base_url, service_name);

    let metadata = McpProtectedResourceMetadata {
        resource: resource_url,
        authorization_servers: vec![base_url.clone()],
        scopes_supported: scopes,
        bearer_methods_supported: vec!["header".to_string()],
        resource_documentation: Some(base_url.clone()),
    };

    (StatusCode::OK, Json(metadata)).into_response()
}

pub async fn handle_mcp_authorization_server(
    Path(_service_name): Path<String>,
) -> impl IntoResponse {
    let base_url = match Config::get() {
        Ok(c) => c.api_external_url.clone(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to get config");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Configuration unavailable"})),
            )
                .into_response();
        },
    };

    let metadata = McpAuthorizationServerMetadata {
        issuer: base_url.clone(),
        authorization_endpoint: format!("{}/api/v1/core/oauth/authorize", base_url),
        token_endpoint: format!("{}/api/v1/core/oauth/token", base_url),
        registration_endpoint: Some(format!("{}/api/v1/core/oauth/register", base_url)),
        scopes_supported: vec!["user".to_string(), "admin".to_string()],
        response_types_supported: vec![ResponseType::Code.to_string()],
        grant_types_supported: vec![
            GrantType::AuthorizationCode.to_string(),
            GrantType::RefreshToken.to_string(),
        ],
        code_challenge_methods_supported: vec![PkceMethod::S256.to_string()],
        token_endpoint_auth_methods_supported: vec![
            TokenAuthMethod::ClientSecretPost.to_string(),
            TokenAuthMethod::ClientSecretBasic.to_string(),
        ],
    };

    (StatusCode::OK, Json(metadata)).into_response()
}

pub(crate) async fn get_mcp_server_scopes(service_name: &str) -> Option<Vec<String>> {
    if McpServerRegistry::validate().is_err() {
        return None;
    }
    let registry = systemprompt_mcp::services::registry::RegistryManager;
    match McpRegistryProvider::get_server(&registry, service_name).await {
        Ok(server_info) if server_info.oauth.required => {
            let scopes: Vec<String> = server_info
                .oauth
                .scopes
                .iter()
                .map(ToString::to_string)
                .collect();
            if scopes.is_empty() {
                None
            } else {
                Some(scopes)
            }
        },
        _ => None,
    }
}

pub fn router(ctx: &AppContext) -> Router {
    let engine = ProxyEngine::new();

    let repo = match ToolUsageRepository::new(ctx.db_pool()) {
        Ok(r) => Arc::new(r),
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize MCP tool usage repository");
            return Router::new();
        },
    };

    let state = McpState {
        ctx: ctx.clone(),
        repo,
    };

    Router::new()
        .route("/executions/{id}", get(handle_get_execution))
        .route(
            "/{service_name}/mcp/.well-known/oauth-protected-resource",
            get(handle_mcp_protected_resource),
        )
        .route(
            "/{service_name}/mcp/.well-known/oauth-authorization-server",
            get(handle_mcp_authorization_server),
        )
        .route(
            "/{service_name}/{*path}",
            any({
                let ctx_clone = ctx.clone();
                move |Path((service_name, path)): Path<(String, String)>, request| {
                    let engine = engine.clone();
                    let ctx = ctx_clone.clone();
                    async move {
                        engine
                            .handle_mcp_request_with_path(
                                Path((service_name, path)),
                                State(ctx),
                                request,
                            )
                            .await
                    }
                }
            }),
        )
        .with_state(state)
}

systemprompt_runtime::register_module_api!(
    "mcp",
    ServiceCategory::Mcp,
    router,
    true,
    systemprompt_runtime::ModuleType::Proxy
);
