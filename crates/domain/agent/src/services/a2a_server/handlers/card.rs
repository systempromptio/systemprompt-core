//! Agent-card request handler for the A2A server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::sync::Arc;
use systemprompt_models::Config;

use super::state::AgentHandlerState;
use crate::error::AgentResult;
use crate::services::registry::AgentRegistry;

pub async fn handle_agent_card(State(state): State<Arc<AgentHandlerState>>) -> impl IntoResponse {
    let config = state.config.read().await;
    let agent_name = config.name.clone();
    drop(config);

    tracing::info!(agent_name = %agent_name, "Fetching agent card");

    let base_url = Config::get().map_or_else(
        |_| "http://localhost:3000".to_owned(),
        |c| c.api_external_url.clone(),
    );

    agent_card_response(AgentRegistry::new(), &agent_name, &base_url).await
}

/// Sibling of [`handle_agent_card`] taking the agent-registry snapshot as an
/// argument instead of resolving the global [`ConfigLoader`] registry.
///
/// [`ConfigLoader`]: systemprompt_loader::ConfigLoader
pub async fn agent_card_response(
    registry: AgentResult<AgentRegistry>,
    agent_name: &str,
    base_url: &str,
) -> Response {
    match registry {
        Ok(registry) => match registry.get_agent(agent_name).await {
            Ok(agent_config) => {
                match registry
                    .to_agent_card(&agent_config.name, base_url, vec![], None)
                    .await
                {
                    Ok(agent_card) => (StatusCode::OK, Json(agent_card)).into_response(),
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build agent card");
                        let error_response = json!({
                            "error": "Internal server error",
                            "message": "Failed to build agent card"
                        });
                        (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
                    },
                }
            },
            Err(_e) => {
                tracing::error!(agent_name = %agent_name, "Agent card not found");
                let error_response = json!({
                    "error": "Agent card not found",
                    "message": format!("No agent card available for agent: {agent_name}")
                });
                (StatusCode::NOT_FOUND, Json(error_response)).into_response()
            },
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to initialize registry");
            let error_response = json!({
                "error": "Internal server error",
                "message": "Failed to initialize agent registry"
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
        },
    }
}
