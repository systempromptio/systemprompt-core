//! `admin keys` command tree: RSA signing-key management for the JWT plane.
//!
//! [`KeysCommands`] generates fresh RS256 keypairs and rotates the
//! deployment's token key in two phases (stage, then promote) with a
//! verification overlap, through the kernel-held signing handle the
//! `signing-keys` grant serves, and mints admin hook credentials
//! (`issue-plugin-token`) through the plugin-token minter conferred on
//! `identity-http`. `identity-http` contributes the group.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generate;
mod plugin_token;
mod rotation;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DescribeCommand,
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
    IssuePluginToken(plugin_token::IssuePluginTokenArgs),
}

pub async fn execute(cmd: KeysCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        KeysCommands::IssuePluginToken(args) => plugin_token::execute(args, host).await,
        KeysCommands::Generate(args) => {
            generate::execute(&args).map(|()| CommandOutput::rendered())
        },
        KeysCommands::Stage(_) => rotation::execute_stage(host),
        KeysCommands::Promote(args) => rotation::execute_promote(args, host),
        KeysCommands::Rotate(args) => rotation::execute_rotate(args, host),
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
        execute(self, host).await.map_err(crate::cli::command_error)
    }
}
