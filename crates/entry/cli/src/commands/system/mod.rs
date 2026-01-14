pub mod types;

mod login;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli_settings::get_global_config;
use crate::shared::render_result;
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum SystemCommands {
    #[command(about = "Create a session and get an authentication token")]
    Login(login::LoginArgs),
}

pub async fn execute(command: SystemCommands) -> Result<()> {
    let config = get_global_config();
    execute_with_config(command, &config).await
}

pub async fn execute_with_config(command: SystemCommands, config: &CliConfig) -> Result<()> {
    match command {
        SystemCommands::Login(args) => {
            let token_only = args.token_only;
            let result = login::execute(args, config)
                .await
                .context("Failed to login")?;
            if !token_only {
                render_result(&result);
            }
            Ok(())
        },
    }
}
