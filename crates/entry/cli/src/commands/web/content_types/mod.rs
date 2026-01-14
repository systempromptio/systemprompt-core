mod create;
mod delete;
mod edit;
mod list;
mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::shared::render_result;
use crate::CliConfig;

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

pub fn execute(command: ContentTypesCommands, config: &CliConfig) -> Result<()> {
    match command {
        ContentTypesCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list content types")?;
            render_result(&result);
            Ok(())
        },
        ContentTypesCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show content type")?;
            render_result(&result);
            Ok(())
        },
        ContentTypesCommands::Create(args) => {
            let result = create::execute(args, config).context("Failed to create content type")?;
            render_result(&result);
            Ok(())
        },
        ContentTypesCommands::Edit(args) => {
            let result = edit::execute(args, config).context("Failed to edit content type")?;
            render_result(&result);
            Ok(())
        },
        ContentTypesCommands::Delete(args) => {
            let result = delete::execute(args, config).context("Failed to delete content type")?;
            render_result(&result);
            Ok(())
        },
    }
}
