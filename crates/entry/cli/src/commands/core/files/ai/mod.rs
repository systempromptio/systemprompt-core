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

use crate::kernel::KernelHost;
use anyhow::{Context, Result};
use clap::Subcommand;

use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Subcommand)]
pub enum AiCommands {
    #[command(about = "List AI-generated images")]
    List(list::ListArgs),

    #[command(about = "Show AI-generated image details")]
    Show(show::ShowArgs),

    #[command(about = "Count AI-generated images")]
    Count(count::CountArgs),
}

pub async fn execute(cmd: AiCommands, host: &dyn KernelHost) -> Result<CommandOutput> {
    match cmd {
        AiCommands::List(args) => {
            list::execute(args, host)
                .await
                .context("Failed to list AI images")
        },
        AiCommands::Show(args) => {
            show::execute(args, host)
                .await
                .context("Failed to show AI image")
        },
        AiCommands::Count(args) => {
            count::execute(args, host)
                .await
                .context("Failed to count AI images")
        },
    }
}
