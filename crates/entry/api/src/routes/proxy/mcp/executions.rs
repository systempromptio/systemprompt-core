//! Tool-execution lookup: `GET /executions/{id}` returns one execution's input
//! and output to the user it belongs to (or an admin). An execution owned by
//! another user answers exactly like a missing one, so ids cannot be probed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;
use systemprompt_identifiers::{McpExecutionId, McpServerId, McpToolName};
use systemprompt_mcp::models::ToolExecution;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_models::auth::UserType;
use systemprompt_models::modules::ApiPaths;
use systemprompt_models::{ApiError, RequestContext};
use systemprompt_runtime::AppContext;

#[derive(Debug, Serialize)]
pub struct ToolExecutionResponse {
    pub id: McpExecutionId,
    pub tool_name: McpToolName,
    pub server_name: McpServerId,
    pub server_endpoint: String,
    // JSON: MCP `tools/call` arguments — schema-less per tool.
    pub input: serde_json::Value,
    // JSON: MCP `tools/call` result — schema-less per tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct ExecutionsState {
    pub repo: Arc<ToolUsageRepository>,
}

pub fn executions_router(ctx: &AppContext) -> Router {
    let repo = crate::repository::tool_usage(ctx.db_pool());
    Router::new()
        .route("/executions/{id}", get(handle_get_execution))
        .with_state(ExecutionsState { repo })
}

fn caller_may_read(req_ctx: &RequestContext, execution: &ToolExecution) -> bool {
    match req_ctx.user_type() {
        UserType::Admin => true,
        UserType::Anon => false,
        _ => execution.user_id == *req_ctx.user_id(),
    }
}

fn not_found() -> Response {
    ApiError::not_found("Execution not found").into_response()
}

async fn handle_get_execution(
    Extension(req_ctx): Extension<RequestContext>,
    Path(raw_execution_id): Path<String>,
    State(state): State<ExecutionsState>,
) -> Response {
    let execution_id = match McpExecutionId::try_new(raw_execution_id) {
        Ok(id) => id,
        Err(error) => return ApiError::from(error).into_response(),
    };
    let execution = match state.repo.find_by_id(&execution_id).await {
        Ok(Some(execution)) if caller_may_read(&req_ctx, &execution) => execution,
        Ok(_) => return not_found(),
        Err(e) => {
            tracing::error!(execution_id = %execution_id, error = %e, "Failed to get execution");
            return ApiError::internal_error("Failed to get execution").into_response();
        },
    };

    let input = match serde_json::from_str(&execution.input) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(execution_id = %execution_id, error = %e, "Invalid input JSON");
            return ApiError::internal_error("Stored execution input is not valid JSON")
                .into_response();
        },
    };

    let output = execution.output.as_deref().and_then(|s| {
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
    });

    Json(ToolExecutionResponse {
        id: execution.mcp_execution_id,
        tool_name: execution.tool_name,
        server_endpoint: ApiPaths::mcp_server_endpoint(execution.server_name.as_str()),
        server_name: execution.server_name,
        input,
        output,
        status: execution.status,
    })
    .into_response()
}
