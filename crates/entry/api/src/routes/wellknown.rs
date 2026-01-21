use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

pub fn wellknown_router(ctx: &AppContext) -> Router {
    Router::new()
        .route(
            ApiPaths::WELLKNOWN_AGENT_CARD,
            get(handle_default_agent_card),
        )
        .route(
            ApiPaths::WELLKNOWN_AGENT_CARDS,
            get(handle_list_agent_cards),
        )
        .route(
            &format!("{}/{{agent_name}}", ApiPaths::WELLKNOWN_AGENT_CARDS),
            get(handle_agent_card_by_name),
        )
        .with_state(ctx.clone())
}

async fn handle_default_agent_card(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = AgentRegistry::new().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal_error("Failed to create agent registry")
    })?;

    let default_agent = registry.get_default_agent().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to get default agent");
        ApiError::not_found("Default agent not found")
    })?;

    let base_url = &ctx.config().api_external_url;

    let agent_card = registry
        .to_agent_card(&default_agent.name, base_url, vec![], None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create agent card");
            ApiError::internal_error("Failed to create agent card")
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_agent_card_by_name(
    State(ctx): State<AppContext>,
    Path(agent_name): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = AgentRegistry::new().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal_error("Failed to create agent registry")
    })?;

    let agent_name = agent_name.trim_end_matches(".json");
    let _agent = registry.get_agent(agent_name).await.map_err(|e| {
        tracing::warn!(agent = %agent_name, error = %e, "Agent not found");
        ApiError::not_found(format!("Agent '{}' not found", agent_name))
    })?;

    let base_url = &ctx.config().api_external_url;

    let agent_card = registry
        .to_agent_card(agent_name, base_url, vec![], None)
        .await
        .map_err(|e| {
            tracing::error!(agent = %agent_name, error = %e, "Failed to create agent card");
            ApiError::internal_error("Failed to create agent card")
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_list_agent_cards(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiError> {
    let registry = AgentRegistry::new().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal_error("Failed to create agent registry")
    })?;

    let agents = registry.list_agents().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list agents");
        ApiError::internal_error("Failed to list agents")
    })?;

    let base_url = &ctx.config().api_external_url;

    let mut cards = Vec::new();
    for agent in agents {
        if let Ok(card) = registry
            .to_agent_card(&agent.name, base_url, vec![], None)
            .await
        {
            cards.push(card);
        }
    }

    Ok(Json(json!(cards)))
}
