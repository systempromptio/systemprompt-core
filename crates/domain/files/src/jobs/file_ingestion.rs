use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::path::Path;
use systemprompt_core_database::DbPool;
use systemprompt_traits::{Job, JobContext, JobResult};
use walkdir::WalkDir;

use crate::{File, FileMetadata, FileRepository, FilesConfig};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "ico"];

#[derive(Debug, Clone, Copy)]
pub struct FileIngestionJob;

impl FileIngestionJob {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FileIngestionJob {
    fn default() -> Self {
        Self::new()
    }
}

struct IngestionStats {
    files_found: u64,
    files_inserted: u64,
    files_skipped: u64,
    errors: u64,
}

struct FileProcessingContext<'a> {
    file_repo: &'a FileRepository,
    files_config: &'a FilesConfig,
    images_dir: &'a Path,
}

#[async_trait]
impl Job for FileIngestionJob {
    fn name(&self) -> &'static str {
        "file_ingestion"
    }

    fn description(&self) -> &'static str {
        "Scans storage directory for image files and creates database entries"
    }

    fn schedule(&self) -> &'static str {
        "0 */30 * * * *"
    }

    async fn execute(&self, ctx: &JobContext) -> Result<JobResult> {
        let start_time = std::time::Instant::now();
        tracing::info!("File ingestion job started");

        let db_pool = ctx
            .db_pool::<DbPool>()
            .ok_or_else(|| anyhow::anyhow!("Database pool not available in job context"))?;

        let files_config = FilesConfig::get()?;
        let images_dir = files_config.storage();

        if !images_dir.exists() {
            tracing::warn!(path = %images_dir.display(), "Images directory not found");
            return Ok(JobResult::success()
                .with_message("Images directory not found")
                .with_duration(start_time.elapsed().as_millis() as u64));
        }

        let file_repo = FileRepository::new(db_pool)?;
        let stats = process_image_files(&file_repo, files_config, images_dir).await;
        let duration_ms = start_time.elapsed().as_millis() as u64;

        tracing::info!(
            files_found = stats.files_found,
            files_inserted = stats.files_inserted,
            files_skipped = stats.files_skipped,
            errors = stats.errors,
            duration_ms = duration_ms,
            "File ingestion job completed"
        );

        Ok(JobResult::success()
            .with_stats(stats.files_inserted, stats.errors)
            .with_message(format!(
                "Found: {}, Inserted: {}, Skipped: {}, Errors: {}",
                stats.files_found, stats.files_inserted, stats.files_skipped, stats.errors
            ))
            .with_duration(duration_ms))
    }
}

async fn process_image_files(
    file_repo: &FileRepository,
    files_config: &FilesConfig,
    images_dir: &Path,
) -> IngestionStats {
    let mut stats = IngestionStats {
        files_found: 0,
        files_inserted: 0,
        files_skipped: 0,
        errors: 0,
    };

    let ctx = FileProcessingContext {
        file_repo,
        files_config,
        images_dir,
    };

    for entry in WalkDir::new(images_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let extension = match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => ext.to_lowercase(),
            None => continue,
        };

        if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
            continue;
        }

        stats.files_found += 1;
        process_single_file(&ctx, path, &extension, &mut stats).await;
    }

    stats
}

async fn process_single_file(
    ctx: &FileProcessingContext<'_>,
    path: &Path,
    extension: &str,
    stats: &mut IngestionStats,
) {
    let file_path = path.to_string_lossy().to_string();
    let public_url = resolve_public_url(ctx, path);

    if check_file_exists(ctx, &file_path, stats).await {
        return;
    }

    let file = build_file_record(&file_path, &public_url, extension, path);
    insert_file_record(ctx, &public_url, file, stats).await;
}

fn resolve_public_url(ctx: &FileProcessingContext<'_>, path: &Path) -> String {
    path.strip_prefix(ctx.images_dir).map_or_else(
        |_| {
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            ctx.files_config.public_url(&file_name)
        },
        |p| ctx.files_config.public_url(&p.to_string_lossy()),
    )
}

async fn check_file_exists(
    ctx: &FileProcessingContext<'_>,
    file_path: &str,
    stats: &mut IngestionStats,
) -> bool {
    match ctx.file_repo.find_by_path(file_path).await {
        Ok(Some(_)) => {
            stats.files_skipped += 1;
            true
        },
        Ok(None) => false,
        Err(e) => {
            tracing::error!(file_path = %file_path, error = %e, "Error checking file existence");
            stats.errors += 1;
            true
        },
    }
}

async fn insert_file_record(
    ctx: &FileProcessingContext<'_>,
    public_url: &str,
    file: File,
    stats: &mut IngestionStats,
) {
    match ctx.file_repo.insert_file(&file).await {
        Ok(_) => stats.files_inserted += 1,
        Err(e) => {
            tracing::error!(file = %public_url, error = %e, "File ingestion failed");
            stats.errors += 1;
        },
    }
}

fn build_file_record(file_path: &str, public_url: &str, extension: &str, path: &Path) -> File {
    let now = Utc::now();
    let metadata =
        serde_json::to_value(FileMetadata::default()).unwrap_or_else(|_| serde_json::json!({}));

    File {
        id: uuid::Uuid::new_v4(),
        path: file_path.to_string(),
        public_url: public_url.to_string(),
        mime_type: mime_from_extension(extension),
        size_bytes: std::fs::metadata(path)
            .map(|m| m.len() as i64)
            .map_err(|e| {
                tracing::debug!(error = %e, path = %path.display(), "Failed to get file size");
                e
            })
            .ok(),
        ai_content: path.to_string_lossy().contains("/generated/"),
        metadata,
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

fn mime_from_extension(ext: &str) -> String {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
    .to_string()
}

systemprompt_provider_contracts::submit_job!(&FileIngestionJob);
