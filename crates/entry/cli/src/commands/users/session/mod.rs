mod cleanup;
mod end;
mod list;

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "List user sessions")]
    List(list::ListArgs),

    #[command(about = "End a user session")]
    End(end::EndArgs),

    #[command(about = "Clean up old anonymous users")]
    Cleanup(cleanup::CleanupArgs),
}

pub async fn execute(cmd: SessionCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SessionCommands::List(args) => list::execute(args, config).await,
        SessionCommands::End(args) => end::execute(args, config).await,
        SessionCommands::Cleanup(args) => cleanup::execute(args, config).await,
    }
}
