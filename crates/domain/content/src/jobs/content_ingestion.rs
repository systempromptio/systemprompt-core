//! Content ingestion job.
//!
//! [`execute_content_ingestion`] walks every enabled content source, resolves
//! each source path against [`systemprompt_models::AppPaths`], and drives the
//! [`IngestionService`] over it, aggregating processed-file and error counts
//! into a [`systemprompt_traits::JobResult`].

use std::path::{Path, PathBuf};
use systemprompt_database::DbPool;
use systemprompt_models::{AppPaths, ContentConfigRaw, ContentSourceConfigRaw};
use systemprompt_traits::JobResult;

use crate::error::{ContentError, ContentResult};
use crate::services::IngestionService;
use crate::{IngestionOptions, IngestionReport, IngestionSource};

pub(in crate::jobs) struct IngestionStats {
    processed: u64,
    errors: u64,
}

pub async fn execute_content_ingestion(
    db_pool: &DbPool,
    content_config: &ContentConfigRaw,
    paths: &AppPaths,
) -> ContentResult<JobResult> {
    let start_time = std::time::Instant::now();
    log_job_started();

    let ingestion_service = create_ingestion_service(db_pool)?;
    let sources = get_enabled_sources(content_config);

    if sources.is_empty() {
        return Ok(empty_sources_result(start_time));
    }

    log_processing_sources(sources.len());
    let stats = process_all_sources(&ingestion_service, &sources, paths).await?;

    Ok(build_result(start_time, &stats))
}

fn log_job_started() {
    tracing::info!("Content ingestion job started");
}

fn create_ingestion_service(db_pool: &DbPool) -> ContentResult<IngestionService> {
    IngestionService::new(db_pool)
        .map_err(|e| ContentError::Service(format!("Failed to create ingestion service: {e}")))
}

fn get_enabled_sources(
    content_config: &ContentConfigRaw,
) -> Vec<(&String, &ContentSourceConfigRaw)> {
    content_config
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
    paths: &AppPaths,
) -> ContentResult<IngestionStats> {
    let mut stats = IngestionStats {
        processed: 0,
        errors: 0,
    };

    for (name, config) in sources {
        process_single_source(service, name, config, paths, &mut stats).await?;
    }

    Ok(stats)
}

async fn process_single_source(
    service: &IngestionService,
    name: &str,
    config: &ContentSourceConfigRaw,
    paths: &AppPaths,
    stats: &mut IngestionStats,
) -> ContentResult<()> {
    tracing::debug!(source = %name, "Ingesting source");

    let content_path = resolve_content_path(&config.path, paths);

    if let Some(err) = validate_source(name, &content_path) {
        stats.errors += 1;
        log_validation_error(&err);
        return Ok(());
    }

    let report = ingest_source(service, name, &content_path, config).await;
    update_stats_from_report(name, report, stats);
    Ok(())
}

fn resolve_content_path(path: &str, paths: &AppPaths) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        paths.system().services().join(path)
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
    source_name: &str,
    path: &Path,
    config: &ContentSourceConfigRaw,
) -> Result<IngestionReport, ContentError> {
    let override_existing = config.indexing.is_some_and(|i| i.override_existing);
    let recursive = config.indexing.is_some_and(|i| i.recursive);
    let source = IngestionSource::new(&config.source_id, source_name, &config.category_id);

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
    report: Result<IngestionReport, ContentError>,
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
