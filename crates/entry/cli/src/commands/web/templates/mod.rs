//! Management of HTML templates and their content-type bindings.
//!
//! Dispatches the `web templates` subcommands ([`TemplatesCommands`]) to list,
//! show, create, edit, and delete templates in the web templates config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod create;
pub mod delete;
pub mod edit;
pub mod list;
pub mod selection;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use std::path::Path;
use systemprompt_extension::cli::{CliConfig, CommandOutput, Prompter};

#[derive(Debug, Subcommand)]
pub enum TemplatesCommands {
    #[command(about = "List all templates")]
    List(list::ListArgs),

    #[command(about = "Show template details")]
    Show(show::ShowArgs),

    #[command(about = "Create a new template")]
    Create(create::CreateArgs),

    #[command(about = "Edit a template")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a template")]
    Delete(delete::DeleteArgs),
}

pub fn execute(
    command: TemplatesCommands,
    prompter: &dyn Prompter,
    config: &CliConfig,
    services_root: &Path,
) -> Result<CommandOutput> {
    match command {
        TemplatesCommands::List(args) => {
            list::execute(args, config, services_root).context("Failed to list templates")
        },
        TemplatesCommands::Show(args) => {
            show::execute(args, prompter, config, services_root).context("Failed to show template")
        },
        TemplatesCommands::Create(args) => create::execute(args, prompter, config, services_root)
            .context("Failed to create template"),
        TemplatesCommands::Edit(args) => {
            edit::execute(args, prompter, config, services_root).context("Failed to edit template")
        },
        TemplatesCommands::Delete(args) => delete::execute(args, prompter, config, services_root)
            .context("Failed to delete template"),
    }
}
