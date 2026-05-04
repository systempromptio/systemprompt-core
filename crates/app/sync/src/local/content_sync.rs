//! Two-way sync between content stored on disk (markdown + frontmatter) and
//! the database via the `systemprompt-content` ingestion + repository layers.

use crate::diff::ContentDiffCalculator;
use crate::error::{SyncError, SyncResult};
use crate::export::export_content_to_file;
use crate::models::{ContentDiffResult, LocalSyncDirection, LocalSyncResult};
use std::path::{Path, PathBuf};
use systemprompt_content::models::{IngestionOptions, IngestionSource};
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::services::IngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};
use tracing::info;

/// One source's worth of inputs into a content sync run.
#[derive(Debug)]
pub struct ContentDiffEntry {
    /// Source-name (e.g. `blog`).
    pub name: String,
    /// Source identifier in the database.
    pub source_id: SourceId,
    /// Content category identifier.
    pub category_id: CategoryId,
    /// Filesystem path holding this source's content.
    pub path: PathBuf,
    /// Allowed `content_type` values; rows with other types are ignored.
    pub allowed_content_types: Vec<String>,
    /// Pre-computed diff for this source.
    pub diff: ContentDiffResult,
}

/// Drives sync between an on-disk content directory and the database.
#[derive(Debug)]
pub struct ContentLocalSync {
    db: DbPool,
}

impl ContentLocalSync {
    /// Construct a new content sync handle.
    pub const fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// Compute the [`ContentDiffResult`] between disk and database for one
    /// source without applying any changes.
    pub async fn calculate_diff(
        &self,
        source_id: &SourceId,
        disk_path: &Path,
        allowed_types: &[String],
    ) -> SyncResult<ContentDiffResult> {
        let calculator = ContentDiffCalculator::new(&self.db).map_err(SyncError::other)?;
        calculator
            .calculate_diff(source_id, disk_path, allowed_types)
            .await
            .map_err(SyncError::other)
    }

    /// Apply the supplied per-source diffs from the database back to disk.
    /// When `delete_orphans` is `true`, on-disk-only files are removed.
    pub async fn sync_to_disk(
        &self,
        diffs: &[ContentDiffEntry],
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        let content_repo = ContentRepository::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDisk,
            ..Default::default()
        };

        for entry in diffs {
            let source_path = &entry.path;

            for item in &entry.diff.modified {
                match content_repo
                    .get_by_source_and_slug(&entry.source_id, &item.slug)
                    .await
                    .map_err(SyncError::other)?
                {
                    Some(content) => {
                        export_content_to_file(&content, source_path, &entry.name)?;
                        result.items_synced += 1;
                        info!("Exported modified content: {}", item.slug);
                    },
                    None => {
                        result
                            .errors
                            .push(format!("Content not found in DB: {}", item.slug));
                    },
                }
            }

            for item in &entry.diff.removed {
                match content_repo
                    .get_by_source_and_slug(&entry.source_id, &item.slug)
                    .await
                    .map_err(SyncError::other)?
                {
                    Some(content) => {
                        export_content_to_file(&content, source_path, &entry.name)?;
                        result.items_synced += 1;
                        info!("Created content on disk: {}", item.slug);
                    },
                    None => {
                        result
                            .errors
                            .push(format!("Content not found in DB: {}", item.slug));
                    },
                }
            }

            if delete_orphans {
                for item in &entry.diff.added {
                    let file_path = if entry.name == "blog" {
                        source_path.join(&item.slug).join("index.md")
                    } else {
                        source_path.join(format!("{}.md", item.slug))
                    };

                    if file_path.exists() {
                        if entry.name == "blog" {
                            std::fs::remove_dir_all(source_path.join(&item.slug))?;
                        } else {
                            std::fs::remove_file(&file_path)?;
                        }
                        result.items_deleted += 1;
                        info!("Deleted orphan content: {}", item.slug);
                    }
                }
            } else {
                result.items_skipped += entry.diff.added.len();
            }
        }

        Ok(result)
    }

    /// Apply the supplied per-source diffs from disk into the database.
    pub async fn sync_to_db(
        &self,
        diffs: &[ContentDiffEntry],
        delete_orphans: bool,
        override_existing: bool,
    ) -> SyncResult<LocalSyncResult> {
        let ingestion_service = IngestionService::new(&self.db).map_err(SyncError::other)?;
        let content_repo = ContentRepository::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDatabase,
            ..Default::default()
        };

        for entry in diffs {
            let source_path = &entry.path;
            let source = IngestionSource::new(&entry.source_id, &entry.name, &entry.category_id);
            let report = ingestion_service
                .ingest_directory(
                    source_path,
                    &source,
                    IngestionOptions::default()
                        .with_recursive(true)
                        .with_override(override_existing),
                )
                .await
                .map_err(SyncError::other)?;

            result.items_synced += report.files_processed;
            result.items_skipped_modified += report.skipped_count;

            for error in report.errors {
                result.errors.push(error);
            }

            info!(
                "Ingested {} files from {}",
                report.files_processed, entry.name
            );

            if delete_orphans {
                for item in &entry.diff.removed {
                    let content_id = ContentId::new(&item.slug);
                    content_repo.delete(&content_id).await.map_err(|e| {
                        SyncError::other(format!("Failed to delete {}: {e}", item.slug))
                    })?;
                    result.items_deleted += 1;
                    info!("Deleted from DB: {}", item.slug);
                }
            } else {
                result.items_skipped += entry.diff.removed.len();
            }
        }

        Ok(result)
    }
}
