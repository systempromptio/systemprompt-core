//! Session-management subcommands for users.
//!
//! [`SessionCommands`] lists active sessions, ends a session, and cleans up old
//! anonymous users. On a `--database-url` invocation only listing is served;
//! the write operations require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cleanup;
mod end;
mod list;

use crate::context::CommandContext;
use crate::shared::render_result;
use anyhow::{Result, bail};
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

pub(super) async fn execute(cmd: SessionCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(cmd, SessionCommands::End(_) | SessionCommands::Cleanup(_))
    {
        bail!("Write operations require full profile context");
    }

    match cmd {
        SessionCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::End(args) => {
            let result = end::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SessionCommands::Cleanup(args) => {
            let result = cleanup::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
