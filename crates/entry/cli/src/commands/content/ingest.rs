use super::types::IngestOutput;
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_content::{IngestionOptions, IngestionService, IngestionSource};
use systemprompt_models::{AppPaths, ContentConfigRaw};
use systemprompt_runtime::AppContext;

const DEFAULT_CATEGORY: &str = "default";

#[derive(Debug, Args)]
pub struct IngestArgs {
    #[arg(help = "Directory path (optional if --source is configured in content config)")]
    pub directory: Option<PathBuf>,

    #[arg(long, help = "Source ID (required)")]
    pub source: String,

    #[arg(long, help = "Category ID")]
    pub category: Option<String>,

    #[arg(
        long,
        help = "Allowed content types (comma-separated, overrides config)"
    )]
    pub allowed_types: Option<String>,

    #[arg(long, help = "Scan recursively")]
    pub recursive: bool,

    #[arg(long, help = "Override existing content")]
    pub r#override: bool,

    #[arg(long, help = "Preview changes without writing to database")]
    pub dry_run: bool,
}

pub async fn execute(args: IngestArgs, _config: &CliConfig) -> Result<CommandResult<IngestOutput>> {
    let directory = resolve_directory(&args)?;

    if !directory.exists() {
        return Err(anyhow!("Directory does not exist: {}", directory.display()));
    }

    if !directory.is_dir() {
        return Err(anyhow!("Path is not a directory: {}", directory.display()));
    }

    let ctx = AppContext::new().await?;
    let service = IngestionService::new(ctx.db_pool())?;

    let allowed_types = resolve_allowed_types(&args)?;
    let category_id = resolve_category_id(&args);

    let allowed_types_refs: Vec<&str> = allowed_types.iter().map(String::as_str).collect();
    let source = IngestionSource::new(&args.source, &category_id, &allowed_types_refs);

    let options = IngestionOptions::default()
        .with_recursive(args.recursive)
        .with_override(args.r#override)
        .with_dry_run(args.dry_run);

    let report = service
        .ingest_directory(&directory, &source, options)
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

fn resolve_directory(args: &IngestArgs) -> Result<PathBuf> {
    if let Some(dir) = &args.directory {
        return Ok(dir.clone());
    }

    let config = load_content_config()?;
    let source_config = config.content_sources.get(&args.source).ok_or_else(|| {
        anyhow!(
            "Source '{}' not found in content config. Provide directory path or configure source.",
            args.source
        )
    })?;

    let content_base = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .system()
        .services()
        .to_path_buf();

    Ok(content_base.join(&source_config.path))
}

fn resolve_allowed_types(args: &IngestArgs) -> Result<Vec<String>> {
    if let Some(types) = &args.allowed_types {
        return Ok(types.split(',').map(|t| t.trim().to_string()).collect());
    }

    let config = load_content_config()?;
    config
        .content_sources
        .get(&args.source)
        .map(|source| source.allowed_content_types.clone())
        .ok_or_else(|| {
            anyhow!(
                "Source '{}' not found in content config. Use --allowed-types to specify types \
                 manually.",
                args.source
            )
        })
}

fn resolve_category_id(args: &IngestArgs) -> String {
    if let Some(category) = &args.category {
        return category.clone();
    }

    let config = load_content_config().ok();
    config
        .and_then(|c| c.content_sources.get(&args.source).cloned())
        .map_or_else(
            || DEFAULT_CATEGORY.to_string(),
            |source| source.category_id.as_str().to_string(),
        )
}

fn load_content_config() -> Result<ContentConfigRaw> {
    let paths = AppPaths::get().map_err(|e| anyhow!("{}", e))?;
    let config_path = paths.system().content_config();
    let yaml_content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read content config: {}", config_path.display()))?;
    serde_yaml::from_str(&yaml_content)
        .with_context(|| format!("Failed to parse content config: {}", config_path.display()))
}
