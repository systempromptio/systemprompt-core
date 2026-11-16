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

use crate::kernel::KernelHost;
use anyhow::{Result, bail};
use clap::Subcommand;
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum SessionCommands {
    #[command(about = "List user sessions")]
    List(list::ListArgs),

    #[command(about = "End a user session")]
    End(end::EndArgs),

    #[command(about = "Clean up old anonymous users")]
    Cleanup(cleanup::CleanupArgs),
}

pub(super) async fn execute(cmd: SessionCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    if host.is_database_scoped()
        && matches!(cmd, SessionCommands::End(_) | SessionCommands::Cleanup(_))
    {
        bail!("Write operations require full profile context");
    }

    match cmd {
        SessionCommands::List(args) => {
            let result = list::execute(args, host).await?;
            Ok(result)
        },
        SessionCommands::End(args) => {
            let result = end::execute(args, host).await?;
            Ok(result)
        },
        SessionCommands::Cleanup(args) => {
            let result = cleanup::execute(args, host).await?;
            Ok(result)
        },
    }
}
