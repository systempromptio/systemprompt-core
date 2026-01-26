use crate::diff::ContentDiffCalculator;
use crate::export::export_content_to_file;
use crate::models::{ContentDiffResult, LocalSyncResult};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use systemprompt_content::models::{IngestionOptions, IngestionSource};
use systemprompt_content::repository::ContentRepository;
use systemprompt_content::services::IngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};
use tracing::info;

#[derive(Debug)]
pub struct ContentDiffEntry {
    pub name: String,
    pub source_id: String,
    pub category_id: String,
    pub path: PathBuf,
    pub allowed_content_types: Vec<String>,
    pub diff: ContentDiffResult,
}

#[derive(Debug)]
pub struct ContentLocalSync {
    db: DbPool,
}

impl ContentLocalSync {
    pub const fn new(db: DbPool) -> Self {
        Self { db }
    }

    pub async fn calculate_diff(
        &self,
        source_id: &str,
        disk_path: &Path,
        allowed_types: &[String],
    ) -> Result<ContentDiffResult> {
        let calculator = ContentDiffCalculator::new(&self.db)?;
        calculator
            .calculate_diff(source_id, disk_path, allowed_types)
            .await
    }

    pub async fn sync_to_disk(
        &self,
        diffs: &[ContentDiffEntry],
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let content_repo = ContentRepository::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: "to_disk".to_string(),
            ..Default::default()
        };

        for entry in diffs {
            let source_path = &entry.path;

            for item in &entry.diff.modified {
                let source_id = SourceId::new(&entry.source_id);
                match content_repo
                    .get_by_source_and_slug(&source_id, &item.slug)
                    .await?
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
                let source_id = SourceId::new(&entry.source_id);
                match content_repo
                    .get_by_source_and_slug(&source_id, &item.slug)
                    .await?
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

    pub async fn sync_to_db(
        &self,
        diffs: &[ContentDiffEntry],
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let ingestion_service = IngestionService::new(&self.db)?;
        let content_repo = ContentRepository::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: "to_database".to_string(),
            ..Default::default()
        };

        for entry in diffs {
            let source_path = &entry.path;
            let source_id = SourceId::new(&entry.source_id);
            let category_id = CategoryId::new(&entry.category_id);
            let source = IngestionSource::new(&source_id, &category_id);
            let report = ingestion_service
                .ingest_directory(
                    source_path,
                    &source,
                    IngestionOptions::default().with_recursive(true),
                )
                .await?;

            result.items_synced += report.files_processed;

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
                    content_repo
                        .delete(&content_id)
                        .await
                        .context(format!("Failed to delete: {}", item.slug))?;
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
