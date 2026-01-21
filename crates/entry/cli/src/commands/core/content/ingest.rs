use super::types::{AllSourcesIngestOutput, IngestOutput, SourceIngestResult};
use crate::cli_settings::CliConfig;
use crate::shared::CommandResult;
use anyhow::{anyhow, Context, Result};
use clap::Args;
use std::path::PathBuf;
use systemprompt_content::{IngestionOptions, IngestionService, IngestionSource};
use systemprompt_models::{AppPaths, ContentConfigRaw, ContentSourceConfigRaw, IndexingConfig};
use systemprompt_runtime::AppContext;

const DEFAULT_CATEGORY: &str = "default";

#[derive(Debug, Args)]
pub struct IngestArgs {
    #[arg(help = "Directory path (optional if --source is configured in content config)")]
    pub directory: Option<PathBuf>,

    #[arg(long, help = "Source ID (required unless --all is used)")]
    pub source: Option<String>,

    #[arg(long, help = "Ingest all enabled content sources from config")]
    pub all: bool,

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

#[derive(Debug)]
pub enum IngestResult {
    Single(CommandResult<IngestOutput>),
    All(CommandResult<AllSourcesIngestOutput>),
}

pub async fn execute(args: IngestArgs, _config: &CliConfig) -> Result<IngestResult> {
    if args.all {
        return execute_all_sources(&args).await;
    }

    let source_id = args
        .source
        .as_ref()
        .ok_or_else(|| anyhow!("--source is required unless --all is specified"))?;

    let directory = resolve_directory(&args, source_id)?;

    if !directory.exists() {
        return Err(anyhow!("Directory does not exist: {}", directory.display()));
    }

    if !directory.is_dir() {
        return Err(anyhow!("Path is not a directory: {}", directory.display()));
    }

    let ctx = AppContext::new().await?;
    let service = IngestionService::new(ctx.db_pool())?;

    let allowed_types = resolve_allowed_types(&args, source_id)?;
    let category_id = resolve_category_id(&args, source_id);
    let indexing_options = resolve_indexing_options(&args, source_id);

    let allowed_types_refs: Vec<&str> = allowed_types.iter().map(String::as_str).collect();
    let source = IngestionSource::new(source_id, &category_id, &allowed_types_refs);

    let options = IngestionOptions::default()
        .with_recursive(indexing_options.recursive)
        .with_override(indexing_options.override_existing)
        .with_dry_run(args.dry_run);

    let report = service
        .ingest_directory(&directory, &source, options)
        .await?;

    let success = report.is_success();
    let output = IngestOutput {
        files_found: report.files_found,
        files_processed: report.files_processed,
        errors: report.errors,
        warnings: report.warnings,
        would_create: report.would_create,
        would_update: report.would_update,
        unchanged_count: report.unchanged_count,
        success,
    };

    Ok(IngestResult::Single(
        CommandResult::card(output).with_title("Ingestion Report"),
    ))
}

async fn execute_all_sources(args: &IngestArgs) -> Result<IngestResult> {
    let config = load_content_config()?;
    let ctx = AppContext::new().await?;
    let service = IngestionService::new(ctx.db_pool())?;

    let content_base = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .system()
        .services()
        .to_path_buf();

    let enabled_sources: Vec<(String, ContentSourceConfigRaw)> = config
        .content_sources
        .into_iter()
        .filter(|(_, source)| source.enabled)
        .filter(|(_, source)| source.sitemap.is_some())
        .collect();

    if enabled_sources.is_empty() {
        return Err(anyhow!("No enabled content sources found in config"));
    }

    let mut source_results = Vec::new();
    let mut total_files_found = 0;
    let mut total_files_processed = 0;
    let mut all_success = true;

    for (name, source_config) in enabled_sources {
        let directory = content_base.join(&source_config.path);

        if !directory.exists() || !directory.is_dir() {
            source_results.push(SourceIngestResult {
                source_id: source_config.source_id.clone(),
                files_found: 0,
                files_processed: 0,
                errors: vec![format!("Directory not found: {}", directory.display())],
                warnings: Vec::new(),
                would_create: Vec::new(),
                would_update: Vec::new(),
                unchanged_count: 0,
                success: false,
            });
            all_success = false;
            continue;
        }

        let allowed_types = source_config.allowed_content_types.clone();
        let category_id = source_config.category_id.as_str().to_string();
        let indexing = source_config
            .indexing
            .unwrap_or_else(IndexingConfig::default);

        let allowed_types_refs: Vec<&str> = allowed_types.iter().map(String::as_str).collect();
        let source = IngestionSource::new(&name, &category_id, &allowed_types_refs);

        let options = IngestionOptions::default()
            .with_recursive(args.recursive || indexing.recursive)
            .with_override(args.r#override || indexing.override_existing)
            .with_dry_run(args.dry_run);

        let report = service
            .ingest_directory(&directory, &source, options)
            .await?;

        total_files_found += report.files_found;
        total_files_processed += report.files_processed;

        if !report.is_success() {
            all_success = false;
        }

        let success = report.is_success();
        source_results.push(SourceIngestResult {
            source_id: source_config.source_id,
            files_found: report.files_found,
            files_processed: report.files_processed,
            errors: report.errors,
            warnings: report.warnings,
            would_create: report.would_create,
            would_update: report.would_update,
            unchanged_count: report.unchanged_count,
            success,
        });
    }

    let output = AllSourcesIngestOutput {
        sources_processed: source_results.len(),
        total_files_found,
        total_files_processed,
        source_results,
        success: all_success,
    };

    Ok(IngestResult::All(
        CommandResult::card(output).with_title("All Sources Ingestion Report"),
    ))
}

fn resolve_directory(args: &IngestArgs, source_id: &str) -> Result<PathBuf> {
    if let Some(dir) = &args.directory {
        return Ok(dir.clone());
    }

    let config = load_content_config()?;
    let source_config = config.content_sources.get(source_id).ok_or_else(|| {
        anyhow!(
            "Source '{}' not found in content config. Provide directory path or configure source.",
            source_id
        )
    })?;

    let content_base = AppPaths::get()
        .map_err(|e| anyhow!("{}", e))?
        .system()
        .services()
        .to_path_buf();

    Ok(content_base.join(&source_config.path))
}

fn resolve_allowed_types(args: &IngestArgs, source_id: &str) -> Result<Vec<String>> {
    if let Some(types) = &args.allowed_types {
        return Ok(types.split(',').map(|t| t.trim().to_string()).collect());
    }

    let config = load_content_config()?;
    config
        .content_sources
        .get(source_id)
        .map(|source| source.allowed_content_types.clone())
        .ok_or_else(|| {
            anyhow!(
                "Source '{}' not found in content config. Use --allowed-types to specify types \
                 manually.",
                source_id
            )
        })
}

fn resolve_category_id(args: &IngestArgs, source_id: &str) -> String {
    if let Some(category) = &args.category {
        return category.clone();
    }

    load_content_config()
        .ok()
        .and_then(|c| c.content_sources.get(source_id).cloned())
        .map_or_else(
            || DEFAULT_CATEGORY.to_string(),
            |source| source.category_id.as_str().to_string(),
        )
}

fn load_content_config() -> Result<ContentConfigRaw> {
    let paths = AppPaths::get().map_err(|e| anyhow!("{}", e))?;
    let config_path = paths.system().content_config();
    let yaml_content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read content config: {}", config_path.display()))?;
    serde_yaml::from_str(&yaml_content)
        .with_context(|| format!("Failed to parse content config: {}", config_path.display()))
}

fn resolve_indexing_options(args: &IngestArgs, source_id: &str) -> IndexingConfig {
    let config_indexing = load_content_config()
        .map_err(|e| {
            tracing::debug!(error = %e, "Failed to load content config, using defaults");
            e
        })
        .ok()
        .and_then(|c| c.content_sources.get(source_id).cloned())
        .and_then(|s| s.indexing)
        .unwrap_or_else(IndexingConfig::default);

    IndexingConfig {
        recursive: args.recursive || config_indexing.recursive,
        override_existing: args.r#override || config_indexing.override_existing,
        clear_before: config_indexing.clear_before,
    }
}
