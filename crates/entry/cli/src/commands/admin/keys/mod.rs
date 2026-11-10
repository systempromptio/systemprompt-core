//! `admin keys` command tree: RSA signing-key management for the JWT plane.
//!
//! [`KeysCommands`] generates fresh RS256 keypairs, rotates the deployment's
//! token key in two phases (stage, then promote) with a verification overlap,
//! and mints long-lived `aud=plugin` tokens used for hook and governance
//! authentication.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generate;
mod issue_plugin_token;
mod rotation;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum KeysCommands {
    #[command(about = "Generate a fresh RSA-2048 signing keypair")]
    Generate(generate::GenerateArgs),

    #[command(
        about = "Stage a new token signing key: verified and published by every restarted \
                 replica, not yet used for signing"
    )]
    Stage(rotation::StageArgs),

    #[command(
        about = "Promote the staged key to signer; the outgoing key keeps verifying for an overlap"
    )]
    Promote(rotation::PromoteArgs),

    #[command(about = "Single-replica rotation: exactly `stage` then `promote`")]
    Rotate(rotation::PromoteArgs),

    #[command(
        name = "issue-plugin-token",
        about = "Mint a long-lived RS256 JWT with `aud=plugin` for hook/governance authentication"
    )]
    IssuePluginToken(issue_plugin_token::IssuePluginTokenArgs),
}

pub async fn execute(cmd: KeysCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        KeysCommands::Generate(args) => generate::execute(&args),
        KeysCommands::Stage(_) => {
            let result = rotation::execute_stage(ctx)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        KeysCommands::Promote(args) => {
            let result = rotation::execute_promote(args, ctx)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        KeysCommands::Rotate(args) => {
            let result = rotation::execute_rotate(args, ctx)?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        KeysCommands::IssuePluginToken(args) => {
            let result = issue_plugin_token::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
    }
}
