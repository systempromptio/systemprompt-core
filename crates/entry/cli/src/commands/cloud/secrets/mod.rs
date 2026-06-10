//! `cloud secrets` subcommand: manage tenant environment secrets in the cloud.
//!
//! Exposes [`SecretsCommands`] (sync, set, unset, cleanup) and dispatches each
//! to the corresponding action in the `commands` submodule, rejecting
//! system-managed keys before they reach the cloud API.

mod commands;
mod helpers;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum SecretsCommands {
    #[command(about = "Sync secrets from profile secrets.json to cloud")]
    Sync,

    #[command(about = "Set secrets (KEY=VALUE pairs)")]
    Set {
        #[arg(required = true)]
        key_values: Vec<String>,
    },

    #[command(about = "Remove secrets")]
    Unset {
        #[arg(required = true)]
        keys: Vec<String>,
    },

    #[command(about = "Remove incorrectly synced system-managed variables")]
    Cleanup,
}

pub(super) async fn execute(cmd: SecretsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        SecretsCommands::Sync => {
            let result = commands::sync_secrets(&ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SecretsCommands::Set { key_values } => {
            let result = commands::set_secrets(key_values, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SecretsCommands::Unset { keys } => {
            let result = commands::unset_secrets(keys, &ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        SecretsCommands::Cleanup => {
            let result = commands::cleanup_secrets(&ctx.cli).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
