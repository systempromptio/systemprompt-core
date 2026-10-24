//! `/.well-known` agent-card endpoints: default card, by-name lookup, listing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::ApiHttpError;
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_identifiers::AgentName;
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
) -> Result<impl IntoResponse, ApiHttpError> {
    let registry = AgentRegistry::new().map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal("Failed to create agent registry", e)
    })?;

    let default_agent = registry.get_default_agent().map_err(|e| {
        tracing::error!(error = %e, "Failed to get default agent");
        ApiError::not_found("Default agent not found")
    })?;

    let base_url = &ctx.config().api_external_url;

    let agent_card = registry
        .to_agent_card(&default_agent.name, base_url, vec![], None)
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create agent card");
            ApiError::internal("Failed to create agent card", e)
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_agent_card_by_name(
    State(ctx): State<AppContext>,
    Path(agent_name): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let agent_name = AgentName::try_new(agent_name.trim_end_matches(".json"))?;
    let registry = AgentRegistry::new().map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal("Failed to create agent registry", e)
    })?;

    let _agent = registry.get_agent(agent_name.as_str()).map_err(|e| {
        tracing::warn!(agent = %agent_name, error = %e, "Agent not found");
        ApiError::not_found(format!("Agent '{}' not found", agent_name))
    })?;

    let base_url = &ctx.config().api_external_url;

    let agent_card = registry
        .to_agent_card(agent_name.as_str(), base_url, vec![], None)
        .map_err(|e| {
            tracing::error!(agent = %agent_name, error = %e, "Failed to create agent card");
            ApiError::internal("Failed to create agent card", e)
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_list_agent_cards(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let registry = AgentRegistry::new().map_err(|e| {
        tracing::error!(error = %e, "Failed to create agent registry");
        ApiError::internal("Failed to create agent registry", e)
    })?;

    let agents = registry.list_agents();

    let base_url = &ctx.config().api_external_url;

    let mut cards = Vec::new();
    for agent in agents {
        if let Ok(card) = registry.to_agent_card(&agent.name, base_url, vec![], None) {
            cards.push(card);
        }
    }

    Ok(Json(json!(cards)))
}
