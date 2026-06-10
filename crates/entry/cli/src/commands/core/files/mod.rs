//! `core files` command group: manage stored file uploads.
//!
//! Dispatches the [`FilesCommands`] subcommands — list/show/search/stats,
//! upload/delete, validate/config, and the `ai` sub-group. On a
//! `--database-url` invocation only the read-only commands are served; upload,
//! delete, validate, config, and ai require a full profile context.

pub mod types;

mod config;
pub(crate) mod delete;
mod list;
mod search;
mod show;
mod stats;
pub mod upload;
mod validate;

pub mod ai;

use anyhow::{Context, Result, bail};
use clap::Subcommand;

use crate::context::CommandContext;
use crate::shared::render_result;

#[derive(Debug, Subcommand)]
pub enum FilesCommands {
    #[command(about = "List files with pagination and filtering")]
    List(list::ListArgs),

    #[command(about = "Show detailed file information")]
    Show(show::ShowArgs),

    #[command(about = "Upload a file from the local filesystem")]
    Upload(upload::UploadArgs),

    #[command(about = "Delete a file")]
    Delete(delete::DeleteArgs),

    #[command(about = "Validate a file before upload")]
    Validate(validate::ValidateArgs),

    #[command(about = "Show file upload configuration")]
    Config(config::ConfigArgs),

    #[command(about = "Search files by path pattern")]
    Search(search::SearchArgs),

    #[command(about = "Show file storage statistics")]
    Stats(stats::StatsArgs),

    #[command(subcommand, about = "AI-generated images operations")]
    Ai(ai::AiCommands),
}

pub async fn execute(cmd: FilesCommands, ctx: &CommandContext) -> Result<()> {
    if ctx.is_database_scoped()
        && matches!(
            cmd,
            FilesCommands::Upload(_)
                | FilesCommands::Delete(_)
                | FilesCommands::Validate(_)
                | FilesCommands::Config(_)
                | FilesCommands::Ai(_)
        )
    {
        bail!("This files command requires full profile context");
    }

    match cmd {
        FilesCommands::List(args) => {
            let result = list::execute(args, ctx)
                .await
                .context("Failed to list files")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Show(args) => {
            let result = show::execute(args, ctx)
                .await
                .context("Failed to show file")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Upload(args) => {
            let result = upload::execute(args, &ctx.cli)
                .await
                .context("Failed to upload file")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Delete(args) => {
            let result = delete::execute(args, &ctx.cli)
                .await
                .context("Failed to delete file")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Validate(args) => {
            let result = validate::execute(&args, &ctx.cli).context("Failed to validate file")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Config(args) => {
            let result = config::execute(args, &ctx.cli).context("Failed to get file config")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Search(args) => {
            let result = search::execute(args, ctx)
                .await
                .context("Failed to search files")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Stats(args) => {
            let result = stats::execute(args, ctx)
                .await
                .context("Failed to get file stats")?;
            render_result(&result, &ctx.cli);
            Ok(())
        },
        FilesCommands::Ai(cmd) => ai::execute(cmd, &ctx.cli).await,
    }
}
