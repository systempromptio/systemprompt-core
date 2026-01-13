use anyhow::{Context, Result};
use clap::Args;

use systemprompt_core_agent::services::a2a_server::run_standalone;

#[derive(Debug, Clone, Args)]
pub struct RunArgs {
    #[arg(long, help = "Agent name to run")]
    pub agent_name: String,

    #[arg(long, help = "Port to listen on")]
    pub port: u16,
}

pub async fn execute(args: RunArgs) -> Result<()> {
    run_standalone(&args.agent_name, args.port)
        .await
        .context("Failed to run agent server")
}
