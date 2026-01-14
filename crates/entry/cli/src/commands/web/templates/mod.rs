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

pub fn execute(command: TemplatesCommands, config: &CliConfig) -> Result<()> {
    match command {
        TemplatesCommands::List(args) => {
            let result = list::execute(args, config).context("Failed to list templates")?;
            render_result(&result);
            Ok(())
        },
        TemplatesCommands::Show(args) => {
            let result = show::execute(args, config).context("Failed to show template")?;
            render_result(&result);
            Ok(())
        },
        TemplatesCommands::Create(args) => {
            let result = create::execute(args, config).context("Failed to create template")?;
            render_result(&result);
            Ok(())
        },
        TemplatesCommands::Edit(args) => {
            let result = edit::execute(args, config).context("Failed to edit template")?;
            render_result(&result);
            Ok(())
        },
        TemplatesCommands::Delete(args) => {
            let result = delete::execute(args, config).context("Failed to delete template")?;
            render_result(&result);
            Ok(())
        },
    }
}
