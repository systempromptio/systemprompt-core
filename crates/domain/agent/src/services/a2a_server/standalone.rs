use anyhow::{Context, Result};
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
        agent_state.db_pool().clone(),
        agent_state,
        ai_service,
        Some(agent_name.to_string()),
        port,
    )
    .await
    .context("Failed to create agent server")?;

    server.run().await.context("Agent server failed")?;

    Ok(())
}
