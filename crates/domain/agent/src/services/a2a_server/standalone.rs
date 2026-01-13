use anyhow::{Context, Result};
use std::sync::Arc;

use systemprompt_core_ai::AiService;
use systemprompt_core_mcp::services::McpToolProvider;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;
use systemprompt_traits::ToolProvider;

use super::Server;

pub async fn run_standalone(agent_name: &str, port: u16) -> Result<()> {
    let app_context = Arc::new(
        AppContext::new()
            .await
            .context("Failed to create app context")?,
    );

    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    let tool_provider: Arc<dyn ToolProvider> =
        Arc::new(McpToolProvider::new(&app_context));

    let ai_service: Arc<dyn systemprompt_models::AiProvider> = Arc::new(
        AiService::new(&app_context, &services_config.ai, tool_provider)
            .context("Failed to create AI service")?,
    );

    let server = Server::new(
        app_context.db_pool().clone(),
        app_context,
        ai_service,
        Some(agent_name.to_string()),
        port,
    )
    .await
    .context("Failed to create agent server")?;

    server.run().await.context("Agent server failed")?;

    Ok(())
}
