mod create;
mod delete;
mod edit;
mod list;
mod new;
mod resolve;
mod show;
mod types;
mod use_context;

use crate::cli_settings::CliConfig;
use crate::shared::render_result;
use anyhow::Result;
use clap::Subcommand;

pub use types::*;

#[derive(Debug, Subcommand)]
pub enum ContextsCommands {
    #[command(about = "List all contexts with stats")]
    List(list::ListArgs),

    #[command(about = "Show context details")]
    Show(show::ShowArgs),

    #[command(about = "Create a new context")]
    Create(create::CreateArgs),

    #[command(about = "Rename a context")]
    Edit(edit::EditArgs),

    #[command(about = "Delete a context")]
    Delete(delete::DeleteArgs),

    #[command(name = "use", about = "Set session's active context")]
    Use(use_context::UseArgs),

    #[command(about = "Create a new context and set it as active")]
    New(new::NewArgs),
}

pub async fn execute(cmd: ContextsCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        ContextsCommands::List(args) => {
            let result = list::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::Show(args) => {
            let result = show::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::Create(args) => {
            let result = create::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::Edit(args) => {
            let result = edit::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::Delete(args) => {
            let result = delete::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::Use(args) => {
            let result = use_context::execute(args, config).await?;
            render_result(&result);
        },
        ContextsCommands::New(args) => {
            let result = new::execute(args, config).await?;
            render_result(&result);
        },
    }
    Ok(())
}
