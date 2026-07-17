//! Standalone A2A server binary entry helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::{AgentServiceError, Result};
use std::sync::Arc;

use systemprompt_models::AiProvider;

use super::Server;
use crate::state::AgentState;

pub async fn run_standalone(
    agent_state: Arc<AgentState>,
    ai_service: Arc<dyn AiProvider>,
    agent_name: &str,
    port: u16,
) -> Result<()> {
    let server = Server::new(
        Arc::clone(agent_state.db_pool()),
        agent_state,
        ai_service,
        Some(agent_name.to_owned()),
        port,
    )
    .await
    .map_err(|e| AgentServiceError::Internal(format!("Failed to create agent server: {e}")))?;

    server
        .run()
        .await
        .map_err(|e| AgentServiceError::Internal(format!("Agent server failed: {e}")))?;

    Ok(())
}
