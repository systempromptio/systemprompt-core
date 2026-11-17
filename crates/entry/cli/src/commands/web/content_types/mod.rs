//! Management of content-source definitions in the web content config.
//!
//! Dispatches the `web content-types` subcommands ([`ContentTypesCommands`]) to
//! list, show, create, edit, and delete the content sources that drive
//! indexing and sitemap generation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod builder;
pub mod create;
mod delete;
pub mod edit;
mod list;
pub mod selection;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::{CliConfig, CommandOutput, Prompter};
use systemprompt_manifest::Profile;

#[derive(Debug, Subcommand)]
pub enum ContentTypesCommands {
    #[command(about = "List all content types")]
    List(list::ListArgs),

    #[command(about = "Show content type details")]
    Show(show::ShowArgs),

    #[command(about = "Create a new content type")]
    Create(create::CreateArgs),

    #[command(about = "Edit a content type")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a content type")]
    Delete(delete::DeleteArgs),
}

pub fn execute(
    command: ContentTypesCommands,
    prompter: &dyn Prompter,
    config: &CliConfig,
    profile: &Profile,
) -> Result<CommandOutput> {
    match command {
        ContentTypesCommands::List(args) => {
            list::execute(&args, config, profile).context("Failed to list content types")
        },
        ContentTypesCommands::Show(args) => {
            show::execute(args, prompter, config, profile).context("Failed to show content type")
        },
        ContentTypesCommands::Create(args) => create::execute(args, prompter, config, profile)
            .context("Failed to create content type"),
        ContentTypesCommands::Edit(args) => {
            edit::execute(&args, prompter, config, profile).context("Failed to edit content type")
        },
        ContentTypesCommands::Delete(args) => delete::execute(args, prompter, config, profile)
            .context("Failed to delete content type"),
    }
}
