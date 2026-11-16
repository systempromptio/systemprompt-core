//! `core files` command group: manage stored file uploads.
//!
//! Dispatches the [`FilesCommands`] subcommands — list/show/search/stats,
//! upload/delete, validate/config, and the `ai` sub-group. On a
//! `--database-url` invocation only the read-only commands are served; upload,
//! delete, validate, config, and ai require a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod types;

mod config;
pub mod delete;
mod list;
mod search;
mod show;
mod stats;
pub mod upload;
mod validate;

pub mod ai;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use systemprompt_config::paths::AppPaths;
use systemprompt_files::FilesConfig;

use systemprompt_extension::cli::{CommandHost, CommandOutput};

fn load_files_config(host: &dyn CommandHost) -> Result<FilesConfig> {
    let profile = host.profile()?;
    let services_root = host.services_root()?;
    let paths = AppPaths::from_profile(
        &profile.paths,
        profile.path_resolution(),
        Some(services_root.path.as_path()),
    )
    .context("Failed to build paths")?;
    Ok(FilesConfig::from_profile(profile, &paths)?)
}

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

pub async fn execute(cmd: FilesCommands, host: &dyn CommandHost) -> Result<CommandOutput> {
    if host.is_database_scoped()
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
            list::execute(args, host)
                .await
                .context("Failed to list files")
        },
        FilesCommands::Show(args) => {
            show::execute(args, host)
                .await
                .context("Failed to show file")
        },
        FilesCommands::Upload(args) => {
            upload::execute(args, host)
                .await
                .context("Failed to upload file")
        },
        FilesCommands::Delete(args) => {
            delete::execute(args, host.prompter(), host)
                .await
                .context("Failed to delete file")
        },
        FilesCommands::Validate(args) => {
            let (result, valid) =
                validate::execute(&args, host).context("Failed to validate file")?;
            if !valid {
                host.render(&result);
                bail!("File failed validation");
            }
            Ok(result)
        },
        FilesCommands::Config(args) => {
            config::execute(args, host).context("Failed to get file config")
        },
        FilesCommands::Search(args) => {
            search::execute(args, host)
                .await
                .context("Failed to search files")
        },
        FilesCommands::Stats(args) => {
            stats::execute(args, host)
                .await
                .context("Failed to get file stats")
        },
        FilesCommands::Ai(cmd) => ai::execute(cmd, host).await,
    }
}
