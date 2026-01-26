use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_database::DbPool;
use systemprompt_models::{AppPaths, ContentConfigRaw, ContentSourceConfigRaw};
use systemprompt_traits::{Job, JobContext, JobResult};

use crate::services::IngestionService;
use crate::{IngestionOptions, IngestionReport, IngestionSource};

#[derive(Debug, Clone, Copy)]
pub struct ContentIngestionJob;

struct IngestionStats {
    processed: u64,
    errors: u64,
}

impl ContentIngestionJob {
    pub async fn execute_ingestion(db_pool: &DbPool) -> Result<JobResult> {
        let start_time = std::time::Instant::now();
        log_job_started();

        let config = load_content_config()?;
        let ingestion_service = create_ingestion_service(db_pool)?;
        let sources = get_enabled_sources(&config);

        if sources.is_empty() {
            return Ok(empty_sources_result(start_time));
        }

        log_processing_sources(sources.len());
        let stats = process_all_sources(&ingestion_service, &sources).await?;

        Ok(build_result(start_time, &stats))
    }
}

fn log_job_started() {
    tracing::info!("Content ingestion job started");
}

fn load_content_config() -> Result<ContentConfigRaw> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let config_path = paths.system().content_config();
    let config_display = config_path.display().to_string();

    let yaml_content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read config: {config_display}"))?;

    serde_yaml::from_str(&yaml_content)
        .with_context(|| format!("Failed to parse config: {config_display}"))
}

fn create_ingestion_service(db_pool: &DbPool) -> Result<IngestionService> {
    IngestionService::new(db_pool)
        .map_err(|e| anyhow::anyhow!("Failed to create ingestion service: {}", e))
}

fn get_enabled_sources(config: &ContentConfigRaw) -> Vec<(&String, &ContentSourceConfigRaw)> {
    config
        .content_sources
        .iter()
        .filter(|(name, cfg)| cfg.enabled && !name.contains("skill"))
        .collect()
}

fn empty_sources_result(start_time: std::time::Instant) -> JobResult {
    tracing::warn!("No enabled content sources found");
    JobResult::success()
        .with_message("No enabled content sources")
        .with_duration(start_time.elapsed().as_millis() as u64)
}

fn log_processing_sources(count: usize) {
    tracing::debug!(sources_count = count, "Processing content sources");
}

async fn process_all_sources(
    service: &IngestionService,
    sources: &[(&String, &ContentSourceConfigRaw)],
) -> Result<IngestionStats> {
    let mut stats = IngestionStats {
        processed: 0,
        errors: 0,
    };

    for (name, config) in sources {
        process_single_source(service, name, config, &mut stats).await?;
    }

    Ok(stats)
}

async fn process_single_source(
    service: &IngestionService,
    name: &str,
    config: &ContentSourceConfigRaw,
    stats: &mut IngestionStats,
) -> Result<()> {
    tracing::debug!(source = %name, "Ingesting source");

    let content_path = resolve_content_path(&config.path)?;

    if let Some(err) = validate_source(name, &content_path) {
        stats.errors += 1;
        log_validation_error(&err);
        return Ok(());
    }

    let report = ingest_source(service, &content_path, config).await;
    update_stats_from_report(name, report, stats);
    Ok(())
}

fn resolve_content_path(path: &str) -> Result<PathBuf> {
    let path = Path::new(path);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(paths.system().services().join(path))
    }
}

enum ValidationError {
    PathNotFound(PathBuf),
}

fn validate_source(_name: &str, path: &Path) -> Option<ValidationError> {
    if !path.exists() {
        return Some(ValidationError::PathNotFound(path.to_path_buf()));
    }
    None
}

fn log_validation_error(err: &ValidationError) {
    match err {
        ValidationError::PathNotFound(p) => {
            tracing::warn!(path = %p.display(), "Source path not found");
        },
    }
}

async fn ingest_source(
    service: &IngestionService,
    path: &Path,
    config: &ContentSourceConfigRaw,
) -> Result<IngestionReport, crate::ContentError> {
    let override_existing = config.indexing.is_some_and(|i| i.override_existing);
    let recursive = config.indexing.is_some_and(|i| i.recursive);
    let source = IngestionSource::new(&config.source_id, &config.category_id);

    service
        .ingest_directory(
            path,
            &source,
            IngestionOptions::default()
                .with_override(override_existing)
                .with_recursive(recursive),
        )
        .await
}

fn update_stats_from_report(
    name: &str,
    report: Result<IngestionReport, crate::ContentError>,
    stats: &mut IngestionStats,
) {
    match report {
        Ok(r) => {
            stats.processed += r.files_processed as u64;
            stats.errors += r.errors.len() as u64;
            log_ingestion_errors(&r.errors);
            log_source_ingested(name, &r);
        },
        Err(e) => {
            tracing::error!(source = %name, error = %e, "Source ingestion failed");
            stats.errors += 1;
        },
    }
}

fn log_ingestion_errors(errors: &[String]) {
    for error in errors {
        tracing::warn!(error = %error, "Ingestion error");
    }
}

fn log_source_ingested(name: &str, report: &IngestionReport) {
    tracing::debug!(
        source = %name,
        files_found = report.files_found,
        files_processed = report.files_processed,
        error_count = report.errors.len(),
        "Source ingested"
    );
}

fn build_result(start_time: std::time::Instant, stats: &IngestionStats) -> JobResult {
    let duration_ms = start_time.elapsed().as_millis() as u64;
    tracing::info!(
        files_processed = stats.processed,
        errors = stats.errors,
        duration_ms = duration_ms,
        "Content ingestion job completed"
    );
    JobResult::success()
        .with_stats(stats.processed, stats.errors)
        .with_duration(duration_ms)
}

#[async_trait::async_trait]
impl Job for ContentIngestionJob {
    fn name(&self) -> &'static str {
        "content_ingestion"
    }

    fn description(&self) -> &'static str {
        "Ingests markdown content from configured directories into the database"
    }

    fn schedule(&self) -> &'static str {
        "0 0 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Failed to get database pool from job context"))?;

        Self::execute_ingestion(pool).await
    }
}

systemprompt_provider_contracts::submit_job!(&ContentIngestionJob);
