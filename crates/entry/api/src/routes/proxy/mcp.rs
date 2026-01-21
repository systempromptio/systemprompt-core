use crate::services::proxy::ProxyEngine;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::ApiError;
use systemprompt_runtime::{AppContext, ServiceCategory};

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
                output: execution
                    .output
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
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
