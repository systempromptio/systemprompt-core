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

use crate::context::CommandContext;
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

pub async fn execute(cmd: ContextsCommands, ctx: &CommandContext) -> Result<()> {
    match cmd {
        ContextsCommands::List(args) => {
            let result = list::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::Show(args) => {
            let result = show::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::Create(args) => {
            let result = create::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::Edit(args) => {
            let result = edit::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::Delete(args) => {
            let result = delete::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::Use(args) => {
            let result = use_context::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
        ContextsCommands::New(args) => {
            let result = new::execute(args, ctx).await?;
            render_result(&result, &ctx.cli);
        },
    }
    Ok(())
}
