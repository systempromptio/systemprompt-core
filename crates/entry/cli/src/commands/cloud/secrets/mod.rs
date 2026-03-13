mod commands;
pub mod helpers;

use anyhow::Result;
use clap::Subcommand;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;

pub use helpers::{load_secrets_json, map_secrets_to_env_vars, sync_cloud_credentials};

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

pub async fn execute(cmd: SecretsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        SecretsCommands::Sync => {
            let result = commands::sync_secrets(config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Set { key_values } => {
            let result = commands::set_secrets(key_values, config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Unset { keys } => {
            let result = commands::unset_secrets(keys, config).await?;
            render_result(&result);
            Ok(())
        },
        SecretsCommands::Cleanup => {
            let result = commands::cleanup_secrets(config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
