//! `admin keys` command tree: RSA signing-key management for the JWT plane.
//!
//! [`KeysCommands`] generates fresh RS256 keypairs, rotates the deployment's
//! token key in two phases (stage, then promote) with a verification overlap,
//! and mints long-lived `aud=plugin` tokens used for hook and governance
//! authentication. Rotation and minting go through the kernel-held
//! signing-keys handle; no key material reaches the command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generate;
mod issue_plugin_token;
mod rotation;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_extension::cli::CommandHost;

use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandOutput, DataImpact, DescribeCommand,
};

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

pub async fn execute(cmd: KeysCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        KeysCommands::Generate(args) => {
            generate::execute(&args).map(|()| CommandOutput::rendered())
        },
        KeysCommands::Stage(_) => {
            let result = rotation::execute_stage(host)?;
            Ok(result)
        },
        KeysCommands::Promote(args) => {
            let result = rotation::execute_promote(args, host)?;
            Ok(result)
        },
        KeysCommands::Rotate(args) => {
            let result = rotation::execute_rotate(args, host)?;
            Ok(result)
        },
        KeysCommands::IssuePluginToken(args) => {
            let result = issue_plugin_token::execute(args, host).await?;
            Ok(result)
        },
    }
}

impl KeysCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Stage(_) | Self::Promote(_) | Self::Rotate(_) => DataImpact::Irreversible,
            Self::Generate(_) | Self::IssuePluginToken(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for KeysCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for KeysCommands {
    const NAME: &'static str = "keys";
    const ABOUT: &'static str = "RSA signing-key generation for the federated JWT plane";

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host).await.map_err(super::command_error)
    }
}
