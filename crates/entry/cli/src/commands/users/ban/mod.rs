mod add;
mod check;
mod cleanup;
mod list;
mod remove;

use crate::cli_settings::CliConfig;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum BanCommands {
    #[command(about = "List active IP bans")]
    List(list::ListArgs),

    #[command(about = "Ban an IP address")]
    Add(add::AddArgs),

    #[command(about = "Remove an IP ban")]
    Remove(remove::RemoveArgs),

    #[command(about = "Check if an IP is banned")]
    Check(check::CheckArgs),

    #[command(about = "Clean up expired bans")]
    Cleanup(cleanup::CleanupArgs),
}

pub async fn execute(cmd: BanCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        BanCommands::List(args) => list::execute(args, config).await,
        BanCommands::Add(args) => add::execute(args, config).await,
        BanCommands::Remove(args) => remove::execute(args, config).await,
        BanCommands::Check(args) => check::execute(args, config).await,
        BanCommands::Cleanup(args) => cleanup::execute(args, config).await,
    }
}
