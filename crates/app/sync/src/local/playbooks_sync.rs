use crate::diff::PlaybooksDiffCalculator;
use crate::export::export_playbook_to_disk;
use crate::models::{LocalSyncResult, PlaybooksDiffResult};
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_agent::repository::content::PlaybookRepository;
use systemprompt_agent::services::PlaybookIngestionService;
use systemprompt_database::DatabaseProvider;
use systemprompt_identifiers::{PlaybookId, SourceId};
use tracing::info;

#[derive(Debug)]
pub struct PlaybooksLocalSync {
    db: Arc<dyn DatabaseProvider>,
    playbooks_path: PathBuf,
}

impl PlaybooksLocalSync {
    pub fn new(db: Arc<dyn DatabaseProvider>, playbooks_path: PathBuf) -> Self {
        Self { db, playbooks_path }
    }

    pub async fn calculate_diff(&self) -> Result<PlaybooksDiffResult> {
        let calculator = PlaybooksDiffCalculator::new(Arc::clone(&self.db));
        calculator.calculate_diff(&self.playbooks_path).await
    }

    pub async fn sync_to_disk(
        &self,
        diff: &PlaybooksDiffResult,
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let playbook_repo = PlaybookRepository::new(Arc::clone(&self.db));
        let mut result = LocalSyncResult {
            direction: "to_disk".to_string(),
            ..Default::default()
        };

        for item in &diff.modified {
            let playbook_id = PlaybookId::new(&item.playbook_id);
            match playbook_repo.get_by_playbook_id(&playbook_id).await? {
                Some(playbook) => {
                    export_playbook_to_disk(&playbook, &self.playbooks_path)?;
                    result.items_synced += 1;
                    info!("Exported modified playbook: {}", item.playbook_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Playbook not found in DB: {}", item.playbook_id));
                },
            }
        }

        for item in &diff.removed {
            let playbook_id = PlaybookId::new(&item.playbook_id);
            match playbook_repo.get_by_playbook_id(&playbook_id).await? {
                Some(playbook) => {
                    export_playbook_to_disk(&playbook, &self.playbooks_path)?;
                    result.items_synced += 1;
                    info!("Created playbook on disk: {}", item.playbook_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Playbook not found in DB: {}", item.playbook_id));
                },
            }
        }

        if delete_orphans {
            for item in &diff.added {
                let domain_parts: Vec<&str> = item.domain.split('/').collect();
                let mut file_dir = self.playbooks_path.join(&item.category);

                for part in domain_parts
                    .iter()
                    .take(domain_parts.len().saturating_sub(1))
                {
                    file_dir = file_dir.join(part);
                }

                let filename = domain_parts.last().unwrap_or(&"");
                let playbook_file = file_dir.join(format!("{}.md", filename));

                if playbook_file.exists() {
                    std::fs::remove_file(&playbook_file)?;

                    let mut current = playbook_file.parent();
                    while let Some(dir) = current {
                        if dir == self.playbooks_path {
                            break;
                        }
                        if let Ok(entries) = std::fs::read_dir(dir) {
                            if entries.count() == 0 {
                                let _ = std::fs::remove_dir(dir);
                            } else {
                                break;
                            }
                        }
                        current = dir.parent();
                    }

                    result.items_deleted += 1;
                    info!("Deleted orphan playbook: {}", item.playbook_id);
                }
            }
        } else {
            result.items_skipped += diff.added.len();
        }

        Ok(result)
    }

    pub async fn sync_to_db(
        &self,
        diff: &PlaybooksDiffResult,
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let ingestion_service = PlaybookIngestionService::new(Arc::clone(&self.db));
        let mut result = LocalSyncResult {
            direction: "to_database".to_string(),
            ..Default::default()
        };

        let source_id = SourceId::new("playbooks");
        let report = ingestion_service
            .ingest_directory(&self.playbooks_path, source_id, true)
            .await?;

        result.items_synced += report.files_processed;

        for error in report.errors {
            result.errors.push(error);
        }

        info!("Ingested {} playbooks", report.files_processed);

        if delete_orphans && !diff.removed.is_empty() {
            tracing::warn!(
                count = diff.removed.len(),
                "Playbook deletion from database not supported, skipping orphan removal"
            );
        }
        result.items_skipped += diff.removed.len();

        Ok(result)
    }
}
