//! `/.well-known` agent-card endpoints: default card, by-name lookup, listing.
//!
//! Handlers read [`AgentCardsState`]: the services view and the open
//! [`AgentCardSite`] the agent member provides. The API merges the router
//! into its well-known router, public and unthrottled as before.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use systemprompt_agent::services::registry::{AgentCardSite, AgentRegistry};
use systemprompt_extension::router::ApiHttpError;
use systemprompt_identifiers::AgentName;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::api::ApiError;
use systemprompt_models::modules::ApiPaths;

/// Router-scoped state of the agent-card routes.
#[derive(Clone, Debug)]
pub struct AgentCardsState {
    pub services: ServicesView,
    pub site: Arc<AgentCardSite>,
}

pub fn router(state: AgentCardsState) -> Router {
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
        .with_state(state)
}

async fn handle_default_agent_card(
    State(state): State<AgentCardsState>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let registry = AgentRegistry::new(&state.services);

    let default_agent = registry.get_default_agent(state.site.cloud).map_err(|e| {
        tracing::error!(error = %e, "Failed to get default agent");
        ApiError::not_found("Default agent not found")
    })?;

    let agent_card = registry
        .to_agent_card(&default_agent.name, state.site.card_site(), vec![], None)
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create agent card");
            ApiError::internal("Failed to create agent card", e)
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_agent_card_by_name(
    State(state): State<AgentCardsState>,
    Path(agent_name): Path<String>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let agent_name =
        AgentName::try_new(agent_name.trim_end_matches(".json")).map_err(ApiError::from)?;
    let registry = AgentRegistry::new(&state.services);

    let _agent = registry.get_agent(agent_name.as_str()).map_err(|e| {
        tracing::warn!(agent = %agent_name, error = %e, "Agent not found");
        ApiError::not_found(format!("Agent '{}' not found", agent_name))
    })?;

    let agent_card = registry
        .to_agent_card(agent_name.as_str(), state.site.card_site(), vec![], None)
        .map_err(|e| {
            tracing::error!(agent = %agent_name, error = %e, "Failed to create agent card");
            ApiError::internal("Failed to create agent card", e)
        })?;

    Ok(Json(json!(agent_card)))
}

async fn handle_list_agent_cards(
    State(state): State<AgentCardsState>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let registry = AgentRegistry::new(&state.services);
    let site = state.site.card_site();

    let cards: Vec<_> = registry
        .list_agents()
        .into_iter()
        .filter_map(|agent| registry.to_agent_card(&agent.name, site, vec![], None).ok())
        .collect();

    Ok(Json(json!(cards)))
}
