//! Standalone A2A server binary entry helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use std::sync::Arc;

use systemprompt_identifiers::AgentName;
use systemprompt_models::AiProvider;

use super::Server;
use crate::state::AgentState;

pub async fn run_standalone(
    agent_state: Arc<AgentState>,
    ai_service: Arc<dyn AiProvider>,
    agent_name: &AgentName,
    port: u16,
) -> Result<()> {
    let server = Server::new(
        Arc::clone(agent_state.db_pool()),
        agent_state,
        ai_service,
        agent_name,
        port,
    )
    .await
    .map_err(|e| AgentServiceError::operation("Failed to create agent server", e))?;

    server
        .run(async {
            if let Err(e) = tokio::signal::ctrl_c().await {
                tracing::error!(error = %e, "Failed to listen for shutdown signal");
            }
        })
        .await
        .map_err(|e| AgentServiceError::operation("Agent server failed", e))?;

    Ok(())
}
