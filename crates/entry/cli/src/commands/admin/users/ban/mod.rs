//! IP-ban management subcommands.
//!
//! [`BanCommands`] covers listing, adding, removing, checking, and cleaning up
//! IP bans. On a `--database-url` invocation only the read-only commands are
//! served; write operations require a full profile context.

mod add;
mod check;
mod cleanup;
mod list;
mod remove;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::{Result, bail};
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

pub(super) async fn execute(cmd: BanCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            cmd,
            BanCommands::Add(_) | BanCommands::Remove(_) | BanCommands::Cleanup(_)
        )
    {
        bail!("Write operations require full profile context");
    }

    match cmd {
        BanCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BanCommands::Add(args) => {
            let result = add::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BanCommands::Remove(args) => {
            let result = remove::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BanCommands::Check(args) => {
            let result = check::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        BanCommands::Cleanup(args) => {
            let result = cleanup::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
