//! `core files` command group: manage stored file uploads.
//!
//! Dispatches the [`FilesCommands`] subcommands — list/show/search/stats,
//! upload/delete, validate/config, and the `ai` sub-group. `files-http`
//! contributes the group under the kernel's `core` group. On a
//! `--database-url` invocation only the read-only commands (list, show,
//! search, stats) are served; upload, delete, validate, config and `ai`
//! require a full profile context and resolve the file repository the
//! `files` member provides and the kernel's open `Arc<FilesConfig>` and
//! `Arc<dyn FileStorage>` seeds by type. Deleting a file is destructive;
//! every other command preserves the data behind the profile.
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

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DataImpact,
    DbUrlRouting, DescribeCommand,
};
use systemprompt_files::{FileRepository, FilesConfig};

async fn files_config(host: &dyn CommandHost) -> Result<Arc<FilesConfig>> {
    Ok(host.resolved().await?.get::<Arc<FilesConfig>>()?)
}

async fn file_repository(host: &dyn CommandHost) -> Result<Arc<FileRepository>> {
    Ok(host.resolved().await?.get::<Arc<FileRepository>>()?)
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
        FilesCommands::List(args) => list::execute(args, host)
            .await
            .context("Failed to list files"),
        FilesCommands::Show(args) => show::execute(args, host)
            .await
            .context("Failed to show file"),
        FilesCommands::Upload(args) => upload::execute(args, host)
            .await
            .context("Failed to upload file"),
        FilesCommands::Delete(args) => delete::execute(args, host.prompter(), host)
            .await
            .context("Failed to delete file"),
        FilesCommands::Validate(args) => {
            let config = files_config(host).await?;
            let (result, valid) =
                validate::execute(&args, &config).context("Failed to validate file")?;
            if !valid {
                return Err(anyhow::Error::new(
                    CommandError::new(anyhow::anyhow!("File failed validation"))
                        .with_output(result),
                ));
            }
            Ok(result)
        },
        FilesCommands::Config(args) => {
            let config = files_config(host).await?;
            Ok(config::execute(args, &config))
        },
        FilesCommands::Search(args) => search::execute(args, host)
            .await
            .context("Failed to search files"),
        FilesCommands::Stats(args) => stats::execute(args, host)
            .await
            .context("Failed to get file stats"),
        FilesCommands::Ai(cmd) => ai::execute(cmd, host).await,
    }
}

impl FilesCommands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Delete(_) => DataImpact::Destructive,
            Self::List(_)
            | Self::Show(_)
            | Self::Upload(_)
            | Self::Validate(_)
            | Self::Config(_)
            | Self::Search(_)
            | Self::Stats(_)
            | Self::Ai(_) => DataImpact::Preserving,
        }
    }
}

impl DescribeCommand for FilesCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::FULL.with_data_impact(self.data_impact())
    }
}

impl CliCommandGroup for FilesCommands {
    const NAME: &'static str = "files";
    const ABOUT: &'static str = "File management and uploads";

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Direct
    }

    async fn run(self, host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        execute(self, host)
            .await
            .map_err(|error| match error.downcast::<CommandError>() {
                Ok(failed) => failed,
                Err(error) => CommandError::new(error),
            })
    }
}
