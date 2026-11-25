//! `admin secrets`: core-held operations over member ciphertext.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod reseal;

use anyhow::Result;
use clap::Subcommand;
use systemprompt_extension::cli::CommandOutput;

use crate::kernel::KernelHost;

#[derive(Debug, Subcommand)]
pub enum SecretsCommands {
    #[command(
        about = "Re-seal a member's declared raw-master-key ciphertext under its member cipher \
                 (report only without --apply)"
    )]
    Reseal(reseal::ResealArgs),
}

pub async fn execute(cmd: SecretsCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        SecretsCommands::Reseal(args) => reseal::execute(args, host).await,
    }
}
