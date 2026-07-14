use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::run_standalone;
use systemprompt_ai::AiService;
use systemprompt_analytics::AnalyticsAiSessionProvider;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::McpToolProvider;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run")]
    pub agent_name: String,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub(super) async fn execute(args: RunArgs) -> Result<()> {
    let ctx = AppContext::new()
        .await
        .context("Failed to bootstrap AppContext for agent subprocess")?;

    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let profile = systemprompt_config::ProfileBootstrap::get()
        .context("Failed to access bootstrapped profile for provider registry")?;
    let db_pool = Arc::clone(ctx.db_pool());

    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );

    let agent_state = Arc::new(AgentState::new(
        Arc::clone(&db_pool),
        Arc::new(ctx.config().clone()),
        jwt_provider,
    ));

    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(&db_pool),
        ctx.mcp_registry().clone(),
        &services_config.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(
        AnalyticsAiSessionProvider::new(&db_pool)
            .context("Failed to create analytics session provider")?,
    );
    let ai_service = Arc::new(
        AiService::new(
            &db_pool,
            &profile.providers,
            &services_config.ai,
            tool_provider,
            Some(session_provider),
        )
        .context("Failed to create AI service")?,
    );

    run_standalone(agent_state, ai_service, &args.agent_name, args.port)
        .await
        .context("Failed to run agent server")
}
