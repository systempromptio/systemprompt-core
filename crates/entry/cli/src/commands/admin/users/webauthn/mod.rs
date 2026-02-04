mod generate_setup_token;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum WebauthnCommands {
    #[command(
        about = "Generate a setup token for linking WebAuthn credentials to an existing user"
    )]
    GenerateSetupToken(generate_setup_token::Args),
}

pub async fn execute(cmd: WebauthnCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        WebauthnCommands::GenerateSetupToken(args) => {
            let result = generate_setup_token::execute(args, config).await?;
            render_result(&result);
            Ok(())
        },
    }
}
