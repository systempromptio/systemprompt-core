//! `admin agent-host`: the kernel's child host for a spawned agent process.
//!
//! The API spawns each agent as
//! `admin agent-host run --agent-name <n> --port <p>` with the host context it
//! grants the process; the path is reserved to the kernel with the `child-host`
//! grant, so no member can hold it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod run;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_extension::cli::CommandOutput;

use crate::kernel::KernelHost;

#[derive(Debug, Subcommand)]
pub enum AgentHostCommands {
    #[command(about = "Run an agent server in this process (spawned by the API)")]
    Run(run::RunArgs),
}

pub async fn execute(command: AgentHostCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        AgentHostCommands::Run(args) => {
            Box::pin(run::execute(args, host))
                .await
                .context("Failed to run agent")?;
            Ok(CommandOutput::rendered())
        },
    }
}
