use anyhow::{Context, Result};
use clap::Args;
use std::sync::Arc;

use systemprompt_agent::services::a2a_server::run_standalone;
use systemprompt_agent::AgentState;
use systemprompt_ai::{AiService, NoopToolProvider};
use systemprompt_database::Database;
use systemprompt_models::Config;
use systemprompt_oauth::JwtValidationProviderImpl;

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run")]
    pub agent_name: String,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub async fn execute(args: RunArgs) -> Result<()> {
    let config = Config::get().context("Failed to get configuration")?;

    let db_pool = Arc::new(
        Database::from_config(&config.database_type, &config.database_url)
            .await
            .context("Failed to connect to database")?,
    );

    let jwt_provider = Arc::new(
        JwtValidationProviderImpl::from_config()
            .context("Failed to create JWT provider")?,
    );

    let agent_state = Arc::new(AgentState::new(
        db_pool.clone(),
        Arc::new(config.clone()),
        jwt_provider,
    ));

    let tool_provider = Arc::new(NoopToolProvider::new());
    let ai_service = Arc::new(
        AiService::new(db_pool, &config.ai, tool_provider, None)
            .context("Failed to create AI service")?,
    );

    run_standalone(agent_state, ai_service, &args.agent_name, args.port)
        .await
        .context("Failed to run agent server")
}
