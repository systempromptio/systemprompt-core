use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_database::ServiceRepository;
use systemprompt_models::{ApiError, CollectionResponse, RequestContext};
use systemprompt_runtime::AppContext;

use systemprompt_agent::models::a2a::{AgentExtension, McpServerMetadata};
use systemprompt_agent::services::registry::AgentRegistry;

pub async fn handle_agent_registry(
    Extension(_req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
) -> impl IntoResponse {
    let registry = match AgentRegistry::new().await {
        Ok(r) => Arc::new(r),
        Err(e) => {
            tracing::error!(error = %e, "Failed to load agent registry");
            return ApiError::internal_error(format!("Failed to load agent registry: {e}"))
                .into_response();
        },
    };
    let service_repo = ServiceRepository::new(ctx.db_pool().clone());
    let api_external_url = &ctx.config().api_external_url;

    match registry.list_agents().await {
        Ok(agents) => {
            let mut agent_cards = Vec::new();

            for agent_config in agents {
                let runtime_status =
                    match service_repo.get_service_by_name(&agent_config.name).await {
                        Ok(Some(service)) => Some((
                            service.status,
                            Some(agent_config.port),
                            service.pid.map(|p| p as u32),
                        )),
                        Ok(None) => Some(("NotStarted".to_string(), Some(agent_config.port), None)),
                        Err(_) => Some(("Unknown".to_string(), Some(agent_config.port), None)),
                    };

                let mcp_extensions = create_mcp_extensions_from_config(
                    &agent_config.metadata.mcp_servers,
                    api_external_url,
                );

                match registry
                    .to_agent_card(
                        &agent_config.name,
                        api_external_url,
                        mcp_extensions,
                        runtime_status,
                    )
                    .await
                {
                    Ok(card) => {
                        agent_cards.push(card);
                    },
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to convert agent to card");
                    },
                }
            }

            agent_cards.sort_by(|a, b| {
                let a_is_default = a
                    .capabilities
                    .extensions
                    .as_ref()
                    .and_then(|exts| exts.iter().find(|e| e.uri == "systemprompt:service-status"))
                    .and_then(|ext| ext.params.as_ref())
                    .and_then(|p| p.get("default"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let b_is_default = b
                    .capabilities
                    .extensions
                    .as_ref()
                    .and_then(|exts| exts.iter().find(|e| e.uri == "systemprompt:service-status"))
                    .and_then(|ext| ext.params.as_ref())
                    .and_then(|p| p.get("default"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                b_is_default.cmp(&a_is_default)
            });

            CollectionResponse::new(agent_cards).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list agents");
            ApiError::internal_error(format!("Failed to retrieve agent registry: {e}"))
                .into_response()
        },
    }
}

fn create_mcp_extensions_from_config(
    server_names: &[String],
    base_url: &str,
) -> Vec<AgentExtension> {
    if server_names.is_empty() {
        return vec![];
    }

    let servers_info: Vec<McpServerMetadata> = server_names
        .iter()
        .map(|name| McpServerMetadata {
            name: name.clone(),
            endpoint: format!("{}/api/v1/mcp/{}/mcp", base_url, name),
            auth: "unknown".to_string(),
            status: "unknown".to_string(),
            version: None,
            tools: None,
        })
        .collect();

    let mcp_protocol_version = "2024-11-05".to_string();

    vec![AgentExtension {
        uri: "systemprompt:mcp-tools".to_string(),
        description: Some("MCP tool execution capabilities with server endpoints".to_string()),
        required: Some(true),
        params: Some(serde_json::json!({
            "supported_protocols": [mcp_protocol_version],
            "servers": servers_info
        })),
    }]
}

pub fn router(ctx: &AppContext) -> Router {
    Router::new()
        .route("/", get(handle_agent_registry))
        .with_state(ctx.clone())
}
