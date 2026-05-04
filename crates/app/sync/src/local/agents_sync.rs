//! Two-way sync between agents stored on disk (one directory per agent) and
//! the database via the `systemprompt-agent` ingestion + repository layers.

use crate::diff::AgentsDiffCalculator;
use crate::error::{SyncError, SyncResult};
use crate::export::export_agent_to_disk;
use crate::models::{AgentsDiffResult, LocalSyncDirection, LocalSyncResult};
use std::path::PathBuf;
use systemprompt_agent::repository::content::AgentRepository;
use systemprompt_agent::services::AgentIngestionService;
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use tracing::info;

/// Drives sync between an on-disk agents directory and the database.
#[derive(Debug)]
pub struct AgentsLocalSync {
    db: DbPool,
    agents_path: PathBuf,
}

impl AgentsLocalSync {
    /// Construct a new sync handle for the given database pool and agents
    /// directory.
    pub const fn new(db: DbPool, agents_path: PathBuf) -> Self {
        Self { db, agents_path }
    }

    /// Compute the [`AgentsDiffResult`] between the configured directory and
    /// the database without applying any changes.
    pub async fn calculate_diff(&self) -> SyncResult<AgentsDiffResult> {
        let calculator = AgentsDiffCalculator::new(&self.db).map_err(SyncError::other)?;
        calculator
            .calculate_diff(&self.agents_path)
            .await
            .map_err(SyncError::other)
    }

    /// Apply `diff` from the database back to disk. When `delete_orphans` is
    /// `true`, on-disk-only agents are deleted.
    pub async fn sync_to_disk(
        &self,
        diff: &AgentsDiffResult,
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        let agent_repo = AgentRepository::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDisk,
            ..Default::default()
        };

        for item in &diff.modified {
            match agent_repo
                .get_by_agent_id(&item.agent_id)
                .await
                .map_err(SyncError::other)?
            {
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
            match agent_repo
                .get_by_agent_id(&item.agent_id)
                .await
                .map_err(SyncError::other)?
            {
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

    /// Apply `diff` from disk into the database via the agent ingestion
    /// service. `delete_orphans` is currently a no-op (agent deletion through
    /// this path is not supported); callers receive a logged WARN.
    pub async fn sync_to_db(
        &self,
        diff: &AgentsDiffResult,
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        let ingestion_service = AgentIngestionService::new(&self.db).map_err(SyncError::other)?;
        let mut result = LocalSyncResult {
            direction: LocalSyncDirection::ToDatabase,
            ..Default::default()
        };

        let source_id = SourceId::new("agents");
        let report = ingestion_service
            .ingest_directory(&self.agents_path, source_id, true)
            .await
            .map_err(SyncError::other)?;

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
