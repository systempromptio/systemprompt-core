use crate::diff::SkillsDiffCalculator;
use crate::export::export_skill_to_disk;
use crate::models::{LocalSyncResult, SkillsDiffResult};
use anyhow::Result;
use std::path::PathBuf;
use systemprompt_agent::repository::content::SkillRepository;
use systemprompt_agent::services::SkillIngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{SkillId, SourceId};
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

    pub async fn calculate_diff(&self) -> Result<SkillsDiffResult> {
        let calculator = SkillsDiffCalculator::new(&self.db)?;
        calculator.calculate_diff(&self.skills_path).await
    }

    pub async fn sync_to_disk(
        &self,
        diff: &SkillsDiffResult,
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let skill_repo = SkillRepository::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: "to_disk".to_string(),
            ..Default::default()
        };

        for item in &diff.modified {
            let skill_id = SkillId::new(&item.skill_id);
            match skill_repo.get_by_skill_id(&skill_id).await? {
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
            let skill_id = SkillId::new(&item.skill_id);
            match skill_repo.get_by_skill_id(&skill_id).await? {
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
                let skill_dir_name = item.skill_id.replace('_', "-");
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
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let ingestion_service = SkillIngestionService::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: "to_database".to_string(),
            ..Default::default()
        };

        let source_id = SourceId::new("skills");
        let report = ingestion_service
            .ingest_directory(&self.skills_path, source_id, true)
            .await?;

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
