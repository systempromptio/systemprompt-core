//! Agent-card listing endpoints with default-first ordering.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use std::sync::Arc;
use systemprompt_database::ServiceRepository;
use systemprompt_models::{ApiError, CollectionResponse, RequestContext};
use systemprompt_runtime::AppContext;

use systemprompt_agent::models::a2a::{AgentCard, AgentExtension, McpServerMetadata};
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::AgentConfig;

pub async fn handle_agent_registry(
    Extension(_req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
) -> impl IntoResponse {
    let registry = match AgentRegistry::new() {
        Ok(r) => Arc::new(r),
        Err(e) => {
            tracing::error!(error = %e, "Failed to load agent registry");
            return ApiError::internal_error(format!("Failed to load agent registry: {e}"))
                .into_response();
        },
    };
    let service_repo = match ServiceRepository::new(ctx.db_pool()) {
        Ok(repo) => repo,
        Err(e) => {
            return ApiError::internal_error(format!("Failed to create service repository: {e}"))
                .into_response();
        },
    };
    let api_external_url = &ctx.config().api_external_url;

    match registry.list_agents().await {
        Ok(agents) => {
            let mut agent_cards =
                build_agent_cards(&registry, &service_repo, api_external_url, agents).await;
            sort_default_first(&mut agent_cards);
            CollectionResponse::new(agent_cards).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list agents");
            ApiError::internal_error(format!("Failed to retrieve agent registry: {e}"))
                .into_response()
        },
    }
}

async fn build_agent_cards(
    registry: &AgentRegistry,
    service_repo: &ServiceRepository,
    api_external_url: &str,
    agents: Vec<AgentConfig>,
) -> Vec<AgentCard> {
    let mut agent_cards = Vec::new();

    for agent_config in agents {
        let runtime_status = match service_repo.find_service_by_name(&agent_config.name).await {
            Ok(Some(service)) => Some((
                service.status,
                Some(agent_config.port),
                service.pid.map(|p| p as u32),
            )),
            Ok(None) => Some(("NotStarted".to_owned(), Some(agent_config.port), None)),
            Err(_) => Some(("Unknown".to_owned(), Some(agent_config.port), None)),
        };

        let mcp_extensions = create_mcp_extensions_from_config(
            &agent_config.metadata.mcp_servers.include,
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

    agent_cards
}

fn is_default_card(card: &AgentCard) -> bool {
    card.capabilities
        .extensions
        .as_ref()
        .and_then(|exts| exts.iter().find(|e| e.uri == "systemprompt:service-status"))
        .and_then(|ext| ext.params.as_ref())
        .and_then(|p| p.get("default"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

fn sort_default_first(agent_cards: &mut [AgentCard]) {
    agent_cards.sort_by_key(|card| std::cmp::Reverse(is_default_card(card)));
}

pub fn create_mcp_extensions_from_config(
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
            auth: "unknown".to_owned(),
            status: "unknown".to_owned(),
            version: None,
            tools: None,
        })
        .collect();

    let mcp_protocol_version = systemprompt_mcp::mcp_protocol_version();

    vec![AgentExtension {
        uri: "systemprompt:mcp-tools".to_owned(),
        description: Some("MCP tool execution capabilities with server endpoints".to_owned()),
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
