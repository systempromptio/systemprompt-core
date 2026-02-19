use crate::diff::AgentsDiffCalculator;
use crate::export::export_agent_to_disk;
use crate::models::{AgentsDiffResult, LocalSyncDirection, LocalSyncResult};
use anyhow::Result;
use std::path::PathBuf;
use systemprompt_agent::repository::content::AgentRepository;
use systemprompt_agent::services::AgentIngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use tracing::info;

#[derive(Debug)]
pub struct AgentsLocalSync {
    db: DbPool,
    agents_path: PathBuf,
}

impl AgentsLocalSync {
    pub const fn new(db: DbPool, agents_path: PathBuf) -> Self {
        Self { db, agents_path }
    }

    pub async fn calculate_diff(&self) -> Result<AgentsDiffResult> {
        let calculator = AgentsDiffCalculator::new(&self.db)?;
        calculator.calculate_diff(&self.agents_path).await
    }

    pub async fn sync_to_disk(
        &self,
        diff: &AgentsDiffResult,
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let agent_repo = AgentRepository::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDisk,
            ..Default::default()
        };

        for item in &diff.modified {
            match agent_repo.get_by_agent_id(&item.agent_id).await? {
                Some(agent) => {
                    export_agent_to_disk(&agent, &self.agents_path)?;
                    result.items_synced += 1;
                    info!("Exported modified agent: {}", item.agent_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Agent not found in DB: {}", item.agent_id));
                },
            }
        }

        for item in &diff.removed {
            match agent_repo.get_by_agent_id(&item.agent_id).await? {
                Some(agent) => {
                    export_agent_to_disk(&agent, &self.agents_path)?;
                    result.items_synced += 1;
                    info!("Created agent on disk: {}", item.agent_id);
                },
                None => {
                    result
                        .errors
                        .push(format!("Agent not found in DB: {}", item.agent_id));
                },
            }
        }

        if delete_orphans {
            for item in &diff.added {
                let agent_dir = self.agents_path.join(&item.name);

                if agent_dir.exists() {
                    std::fs::remove_dir_all(&agent_dir)?;
                    result.items_deleted += 1;
                    info!("Deleted orphan agent: {}", item.agent_id);
                }
            }
        } else {
            result.items_skipped += diff.added.len();
        }

        Ok(result)
    }

    pub async fn sync_to_db(
        &self,
        diff: &AgentsDiffResult,
        delete_orphans: bool,
    ) -> Result<LocalSyncResult> {
        let ingestion_service = AgentIngestionService::new(&self.db)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDatabase,
            ..Default::default()
        };

        let source_id = SourceId::new("agents");
        let report = ingestion_service
            .ingest_directory(&self.agents_path, source_id, true)
            .await?;

        result.items_synced += report.files_processed;

        for error in report.errors {
            result.errors.push(error);
        }

        info!("Ingested {} agents", report.files_processed);

        if delete_orphans && !diff.removed.is_empty() {
            tracing::warn!(
                count = diff.removed.len(),
                "Agent deletion from database not supported, skipping orphan removal"
            );
        }
        result.items_skipped += diff.removed.len();

        Ok(result)
    }
}
