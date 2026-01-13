pub mod types;

mod config;
mod delete;
mod list;
mod show;
pub mod upload;
mod validate;

pub mod ai;
pub mod content;

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Subcommand;
use systemprompt_core_database::{Database, DbPool};
use systemprompt_models::SecretsBootstrap;

use crate::shared::render_result;
use crate::CliConfig;

pub async fn create_db_pool() -> Result<DbPool> {
    let url = SecretsBootstrap::database_url()?.to_string();
    let database = Database::from_config("postgres", &url)
        .await
        .context("Failed to connect to database")?;
    Ok(Arc::new(database))
}

#[derive(Debug, Subcommand)]
pub enum FilesCommands {
    #[command(about = "List files with pagination and filtering")]
    List(list::ListArgs),

    #[command(about = "Show detailed file information")]
    Show(show::ShowArgs),

    #[command(about = "Upload a file from the local filesystem")]
    Upload(upload::UploadArgs),

    #[command(about = "Delete a file (soft delete)")]
    Delete(delete::DeleteArgs),

    #[command(about = "Validate a file before upload")]
    Validate(validate::ValidateArgs),

    #[command(about = "Show file upload configuration")]
    Config(config::ConfigArgs),

    #[command(subcommand, about = "Content-file linking operations")]
    Content(content::ContentCommands),

    #[command(subcommand, about = "AI-generated images operations")]
    Ai(ai::AiCommands),
}

pub async fn execute(cmd: FilesCommands, config: &CliConfig) -> Result<()> {
    execute_with_config(cmd, config).await
}

pub async fn execute_with_config(cmd: FilesCommands, config: &CliConfig) -> Result<()> {
    match cmd {
        FilesCommands::List(args) => {
            let result = list::execute(args, config)
                .await
                .context("Failed to list files")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Show(args) => {
            let result = show::execute(args, config)
                .await
                .context("Failed to show file")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Upload(args) => {
            let result = upload::execute(args, config)
                .await
                .context("Failed to upload file")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Delete(args) => {
            let result = delete::execute(args, config)
                .await
                .context("Failed to delete file")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Validate(args) => {
            let result = validate::execute(args, config).context("Failed to validate file")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Config(args) => {
            let result = config::execute(args, config).context("Failed to get file config")?;
            render_result(&result);
            Ok(())
        },
        FilesCommands::Content(cmd) => content::execute(cmd, config).await,
        FilesCommands::Ai(cmd) => ai::execute(cmd, config).await,
    }
}
