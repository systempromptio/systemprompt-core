//! `admin keys` command tree: RSA signing-key management for the JWT plane.
//!
//! [`KeysCommands`] generates fresh RS256 keypairs and mints long-lived
//! `aud=plugin` tokens used for hook and governance authentication.

mod generate;
mod issue_plugin_token;

use anyhow::Result;
use clap::Subcommand;

use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum KeysCommands {
    #[command(about = "Generate a fresh RSA-2048 signing keypair")]
    Generate(generate::GenerateArgs),

    #[command(
        name = "issue-plugin-token",
        about = "Mint a long-lived RS256 JWT with `aud=plugin` for hook/governance authentication"
    )]
    IssuePluginToken(issue_plugin_token::IssuePluginTokenArgs),
}

pub async fn execute(cmd: KeysCommands) -> Result<()> {
    match cmd {
        KeysCommands::Generate(args) => generate::execute(args),
        KeysCommands::IssuePluginToken(args) => {
            let result = issue_plugin_token::execute(args).await?;
            render_result(&result);
            Ok(())
        },
    }
}
