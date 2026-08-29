//! `core files ai` command group: inspect AI-generated image files.
//!
//! Dispatches the [`AiCommands`] subcommands (list, show, count) over files
//! flagged as AI-generated content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod count;
pub mod list;
pub mod show;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum AiCommands {
    #[command(about = "List AI-generated images")]
    List(list::ListArgs),

    #[command(about = "Show AI-generated image details")]
    Show(show::ShowArgs),

    #[command(about = "Count AI-generated images")]
    Count(count::CountArgs),
}

pub async fn execute(cmd: AiCommands, ctx: &CommandContext) -> Result<()> {
    let config = &ctx.cli;
    match cmd {
        AiCommands::List(args) => {
            let result = list::execute(args, ctx)
                .await
                .context("Failed to list AI images")?;
            render_result(&result, config);
            Ok(())
        },
        AiCommands::Show(args) => {
            let result = show::execute(args, ctx)
                .await
                .context("Failed to show AI image")?;
            render_result(&result, config);
            Ok(())
        },
        AiCommands::Count(args) => {
            let result = count::execute(args, ctx)
                .await
                .context("Failed to count AI images")?;
            render_result(&result, config);
            Ok(())
        },
    }
}
