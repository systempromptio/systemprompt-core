//! Two-way sync between skills stored on disk and the database via the
//! `systemprompt-agent` skill ingestion + repository layers.

use crate::diff::SkillsDiffCalculator;
use crate::error::{SyncError, SyncResult};
use crate::export::export_skill_to_disk;
use crate::models::{LocalSyncDirection, LocalSyncResult, SkillsDiffResult};
use std::path::PathBuf;
use systemprompt_agent::repository::content::SkillRepository;
use systemprompt_agent::services::SkillIngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_models::SkillsConfig;
use tracing::info;

#[derive(Debug)]
pub struct SkillsLocalSync {
    db: DbPool,
    skills_path: PathBuf,
}

impl SkillsLocalSync {
    pub const fn new(db: DbPool, skills_path: PathBuf) -> Self {
        Self { db, skills_path }
    }

    pub async fn calculate_diff(&self) -> SyncResult<SkillsDiffResult> {
        let calculator = SkillsDiffCalculator::new(&self.db).map_err(SyncError::other)?;
        calculator
            .calculate_diff(&self.skills_path)
            .await
            .map_err(SyncError::other)
    }

    pub async fn sync_to_disk(
        &self,
        diff: &SkillsDiffResult,
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        let skill_repo = SkillRepository::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDisk,
            ..Default::default()
        };

        for item in &diff.modified {
            match skill_repo
                .get_by_skill_id(&item.skill_id)
                .await
                .map_err(SyncError::other)?
            {
                Some(skill) => {
                    export_skill_to_disk(&skill, &self.skills_path)?;
                    result.items_synced += 1;
                    info!("Exported modified skill: {}", item.skill_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Skill not found in DB: {}", item.skill_id));
                },
            }
        }

        for item in &diff.removed {
            match skill_repo
                .get_by_skill_id(&item.skill_id)
                .await
                .map_err(SyncError::other)?
            {
                Some(skill) => {
                    export_skill_to_disk(&skill, &self.skills_path)?;
                    result.items_synced += 1;
                    info!("Created skill on disk: {}", item.skill_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Skill not found in DB: {}", item.skill_id));
                },
            }
        }

        if delete_orphans {
            for item in &diff.added {
                let skill_dir_name = item.skill_id.as_str().replace('_', "-");
                let skill_dir = self.skills_path.join(&skill_dir_name);

                if skill_dir.exists() {
                    std::fs::remove_dir_all(&skill_dir)?;
                    result.items_deleted += 1;
                    info!("Deleted orphan skill: {}", item.skill_id);
                }
            }
        } else {
            result.items_skipped += diff.added.len();
        }

        Ok(result)
    }

    pub async fn sync_to_db(
        &self,
        diff: &SkillsDiffResult,
        skills_config: &SkillsConfig,
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        let ingestion_service = SkillIngestionService::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDatabase,
            ..Default::default()
        };

        let source_id = SourceId::new("skills");
        let report = ingestion_service
            .ingest_config(skills_config, source_id, true)
            .await
            .map_err(SyncError::other)?;

        result.items_synced += report.files_processed;

        for error in report.errors {
            result.errors.push(error);
        }

        info!("Ingested {} skills", report.files_processed);

        if delete_orphans && !diff.removed.is_empty() {
            tracing::warn!(
                count = diff.removed.len(),
                "Skill deletion from database not supported, skipping orphan removal"
            );
        }
        result.items_skipped += diff.removed.len();

        Ok(result)
    }
}
