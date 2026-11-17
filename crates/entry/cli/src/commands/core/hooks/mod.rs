//! `core hooks` command group: list and validate plugin hook definitions.
//!
//! Dispatches the [`HooksCommands`] subcommands (list, validate) that enumerate
//! hooks across installed plugins and check their definitions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod list;
pub mod validate;

use crate::kernel::KernelHost;
use anyhow::{Context, Result, bail};
use clap::Subcommand;

use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum HooksCommands {
    #[command(about = "List hooks across all plugins")]
    List(list::ListArgs),

    #[command(about = "Validate all hook definitions")]
    Validate(validate::ValidateArgs),
}

pub fn execute(command: HooksCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        HooksCommands::List(args) => list::execute(args, host).context("Failed to list hooks"),
        HooksCommands::Validate(args) => {
            let (result, valid) =
                validate::execute(args, host).context("Failed to validate hooks")?;
            if !valid {
                host.render(&result);
                bail!("Hook validation failed");
            }
            Ok(result)
        },
    }
}
