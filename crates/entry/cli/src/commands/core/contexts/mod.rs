//! `core contexts` command group: manage conversation contexts.
//!
//! Dispatches the [`ContextsCommands`] subcommands (list, show, create, edit,
//! delete, use, new) that create contexts and set the session's active one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod create;
pub mod delete;
pub mod edit;
mod list;
pub mod new;
pub mod resolve;
pub mod show;
mod types;
pub mod use_context;

use systemprompt_extension::cli::{CommandHost, CommandOutput};
use anyhow::Result;
use clap::Subcommand;

pub use types::{
    ContextCreatedOutput, ContextDeletedOutput, ContextListOutput, ContextSummary,
    ContextSwitchedOutput, ContextUpdatedOutput,
};

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

pub async fn execute(cmd: ContextsCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    match cmd {
        ContextsCommands::List(args) => Box::pin(list::execute(args, host)).await,
        ContextsCommands::Show(args) => Box::pin(show::execute(args, host)).await,
        ContextsCommands::Create(args) => Box::pin(create::execute(args, host)).await,
        ContextsCommands::Edit(args) => Box::pin(edit::execute(args, host)).await,
        ContextsCommands::Delete(args) => Box::pin(delete::execute(args, host)).await,
        ContextsCommands::Use(args) => Box::pin(use_context::execute(args, host)).await,
        ContextsCommands::New(args) => Box::pin(new::execute(args, host)).await,
    }
}
