//! `admin agents run` command hosting one agent process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use systemprompt_agent::AgentState;
use systemprompt_agent::services::a2a_server::run_standalone;
use systemprompt_agent::services::a2a_server::streaming::webhook_client::HttpWebhookBroadcaster;
use systemprompt_ai::{AiService, AiServiceProviders};
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::McpToolProvider;
use systemprompt_oauth::JwtValidationProviderImpl;
use systemprompt_runtime::AppContext;
use systemprompt_users::UsersAiSessionProvider;

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
    let db_pool = Arc::clone(ctx.db_pool());

    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config().context("Failed to create JWT provider")?,
    );

    let agent_state = Arc::new(AgentState::new(
        Arc::clone(&db_pool),
        Arc::new(ctx.config().clone()),
        jwt_provider,
        Arc::clone(ctx.a2a_repositories()),
        Arc::new(HttpWebhookBroadcaster::from_config(ctx.config())?),
    ));

    let tool_provider = Arc::new(McpToolProvider::new(
        Arc::clone(&db_pool),
        ctx.mcp_registry().clone(),
        &services_config.ai.mcp.resilience,
    ));
    let session_provider = Arc::new(UsersAiSessionProvider::from_repository(
        systemprompt_users::SessionRepository::new(&db_pool)?,
    ));
    let ai_service = Arc::new(
        AiService::new(
            &db_pool,
            &services_config.providers,
            &services_config.ai,
            AiServiceProviders {
                tools: tool_provider,
                sessions: session_provider,
            },
            ctx.ai_repositories(),
        )
        .context("Failed to create AI service")?
        .with_context_materializer(ctx.context_materializer()),
    );

    let provider: Arc<dyn systemprompt_models::AiProvider> = Arc::<AiService>::clone(&ai_service);
    let served = run_standalone(agent_state, provider, &args.agent_name, args.port)
        .await
        .context("Failed to run agent server");
    ai_service.audit_tasks().close();
    ai_service.audit_tasks().wait().await;
    served
}
