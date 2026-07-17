//! YAML → DB sync for access-control baseline rules.
//!
//! Reads an [`AccessControlConfig`] from disk and projects it into
//! `access_control_rules` via
//! [`AccessControlIngestionService`](systemprompt_security::authz::AccessControlIngestionService).
//!
//! Direction is fixed: YAML drives the DB. The `to_disk` direction does
//! not exist for ACL — DB→YAML promotion is an operator-explicit one-shot
//! export from the CLI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_database::DbPool;
use systemprompt_security::authz::{
    AccessControlConfig, AccessControlIngestionService, IngestOptions,
};

use crate::error::{SyncError, SyncResult};
use crate::models::{LocalSyncDirection, LocalSyncResult};

#[derive(Debug)]
pub struct AccessControlLocalSync {
    db: DbPool,
    yaml_path: PathBuf,
}

impl AccessControlLocalSync {
    pub const fn new(db: DbPool, yaml_path: PathBuf) -> Self {
        Self { db, yaml_path }
    }

    pub async fn sync_to_db(
        &self,
        override_existing: bool,
        delete_orphans: bool,
    ) -> SyncResult<LocalSyncResult> {
        if !self.yaml_path.exists() {
            return Err(SyncError::MissingConfig(format!(
                "Access-control config not found at: {}",
                self.yaml_path.display()
            )));
        }

        let raw = std::fs::read_to_string(&self.yaml_path).map_err(|e| {
            SyncError::internal(format!(
                "Failed to read {}: {}",
                self.yaml_path.display(),
                e
            ))
        })?;
        let config: AccessControlConfig = serde_yaml::from_str(&raw).map_err(|e| {
            SyncError::invalid_input(format!(
                "Failed to parse {} as AccessControlConfig: {}",
                self.yaml_path.display(),
                e
            ))
        })?;

        let options = IngestOptions {
            override_existing,
            delete_orphans,
        };

        let svc = AccessControlIngestionService::new(&self.db).map_err(SyncError::internal)?;
        let report = svc
            .ingest_config(&config, options)
            .await
            .map_err(SyncError::internal)?;

        Ok(LocalSyncResult {
            items_synced: report.inserted + report.updated,
            items_skipped: report.skipped,
            items_skipped_modified: 0,
            items_deleted: report.deleted,
            errors: Vec::new(),
            direction: LocalSyncDirection::ToDatabase,
        })
    }
}
