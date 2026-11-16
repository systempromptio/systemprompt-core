//! `WebAuthn` credential-management subcommands.
//!
//! [`WebauthnCommands`] generates a setup token that links a passkey credential
//! to an existing user, dispatched by [`execute`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generate_setup_token;

use anyhow::Result;
use clap::Subcommand;
use crate::shared::CommandOutput;
use systemprompt_extension::cli::CommandHost;

#[derive(Debug, Subcommand)]
pub enum WebauthnCommands {
    #[command(
        about = "Generate a setup token for linking WebAuthn credentials to an existing user"
    )]
    GenerateSetupToken(generate_setup_token::Args),
}

pub(super) async fn execute(cmd: WebauthnCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        WebauthnCommands::GenerateSetupToken(args) => {
            let result = generate_setup_token::execute(args, host).await?;
            Ok(result)
        },
    }
}
