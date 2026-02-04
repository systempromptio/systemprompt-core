mod add;
mod check;
mod cleanup;
mod list;
mod remove;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::{bail, Result};
use clap::Subcommand;
use systemprompt_database::DbPool;

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
        BanCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Add(args) => {
            let result = add::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Remove(args) => {
            let result = remove::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Check(args) => {
            let result = check::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Cleanup(args) => {
            let result = cleanup::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}

pub async fn execute_with_pool(cmd: BanCommands, pool: &DbPool, config: &CliConfig) -> Result<()> {
    match cmd {
        BanCommands::List(args) => {
            let result = list::execute_with_pool(args, pool, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Check(args) => {
            let result = check::execute_with_pool(args, pool, config).await?;
            render_result(&result);
            Ok(())
        },
        BanCommands::Add(_) | BanCommands::Remove(_) | BanCommands::Cleanup(_) => {
            bail!("Write operations require full profile context")
        },
    }
}
