//! `admin identity` command tree: the secrets every replica must share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod generate;

use anyhow::Result;
use clap::Subcommand;

use crate::context::CommandContext;

#[derive(Clone, Copy, Debug, Subcommand)]
pub enum IdentityCommands {
    #[command(
        about = "Mint oauth_at_rest_pepper, manifest_signing_secret_seed and signing_key_pem in \
                 the encodings the secrets loader expects; never writes a file"
    )]
    Generate(generate::GenerateArgs),
}

pub fn execute(cmd: IdentityCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        IdentityCommands::Generate(args) => generate::execute(args, ctx),
    }
}
