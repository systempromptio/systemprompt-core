use super::types::IngestOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_content::{IngestionOptions, IngestionService, IngestionSource};
use systemprompt_runtime::AppContext;

const DEFAULT_CATEGORY: &str = "default";
const ALLOWED_CONTENT_TYPES: &[&str] = &["article", "paper", "guide", "tutorial"];

#[derive(Debug, Args)]
pub struct IngestArgs {
    #[arg(help = "Directory path")]
    pub directory: PathBuf,

    #[arg(long, help = "Source ID (required)")]
    pub source: String,

    #[arg(long, help = "Category ID")]
    pub category: Option<String>,

    #[arg(long, help = "Scan recursively")]
    pub recursive: bool,

    #[arg(long, help = "Override existing content")]
    pub r#override: bool,

    #[arg(long, help = "Preview changes without writing to database")]
    pub dry_run: bool,
}

pub async fn execute(args: IngestArgs, _config: &CliConfig) -> Result<CommandResult<IngestOutput>> {
    if !args.directory.exists() {
        return Err(anyhow!(
            "Directory does not exist: {}",
            args.directory.display()
        ));
    }

    if !args.directory.is_dir() {
        return Err(anyhow!(
            "Path is not a directory: {}",
            args.directory.display()
        ));
    }

    let ctx = AppContext::new().await?;
    let service = IngestionService::new(ctx.db_pool())?;

    let category_id = args.category.as_deref().unwrap_or(DEFAULT_CATEGORY);

    let source = IngestionSource::new(&args.source, category_id, ALLOWED_CONTENT_TYPES);

    let options = IngestionOptions::default()
        .with_recursive(args.recursive)
        .with_override(args.r#override)
        .with_dry_run(args.dry_run);

    let report = service
        .ingest_directory(&args.directory, &source, options)
        .await?;

    let success = report.is_success();
    let output = IngestOutput {
        files_found: report.files_found,
        files_processed: report.files_processed,
        errors: report.errors,
        success,
    };

    Ok(CommandResult::card(output).with_title("Ingestion Report"))
}
