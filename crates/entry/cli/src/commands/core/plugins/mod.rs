//! `plugins` CLI command group: list, show, validate, and generate plugins.
//!
//! [`PluginsCommands`] enumerates the subcommands; [`execute`] dispatches each
//! to its submodule and renders the result. Output payload types live in
//! [`types`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

pub mod generate;
pub mod list;
pub mod show;
pub mod validate;

use anyhow::{Context, Result, bail};
use crate::kernel::KernelHost;
use clap::Subcommand;

use systemprompt_extension::cli::{CommandOutput};

#[derive(Debug, Subcommand)]
pub enum PluginsCommands {
    #[command(about = "List configured plugins")]
    List(list::ListArgs),

    #[command(about = "Show plugin details")]
    Show(show::ShowArgs),

    #[command(about = "Validate plugin configuration")]
    Validate(validate::ValidateArgs),

    #[command(about = "Generate Claude Code plugin output")]
    Generate(generate::GenerateArgs),
}

pub fn execute(command: PluginsCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match command {
        PluginsCommands::List(args) => {
            list::execute(args, host).context("Failed to list plugins")
        },
        PluginsCommands::Show(args) => {
            show::execute(&args, host).context("Failed to show plugin")
        },
        PluginsCommands::Validate(args) => {
            let (result, valid) =
                validate::execute(args, host).context("Failed to validate plugins")?;
            if !valid {
                host.render(&result);
                bail!("Plugin validation failed");
            }
            Ok(result)
        },
        PluginsCommands::Generate(args) => {
            generate::execute(&args, host).context("Failed to generate plugins")
        },
    }
}
