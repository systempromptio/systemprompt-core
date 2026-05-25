//! Cloud sync orchestration for systemprompt.io.
//!
//! Drives push/pull of files, agents, content, and database state
//! between a local systemprompt project and the systemprompt cloud (or a
//! self-hosted tenant in direct-sync mode).
//!
//! # Public surface
//!
//! - [`SyncService`], [`SyncConfig`], [`SyncConfigBuilder`] — high-level façade
//!   that wires everything together for `cloud sync` commands.
//! - [`SyncApiClient`] — low-level HTTP client for the cloud API.
//! - [`ContentLocalSync`] — disk ↔ database sync for content.
//! - [`ContentDiffCalculator`] — pure diff computation.
//! - [`SyncError`] / [`SyncResult`] — typed error returned by every public
//!   function in this crate.
//!
//! # Feature flags
//!
//! This crate has no Cargo features.

mod config;
mod result;

pub mod api_client;
pub mod crate_deploy;
pub mod database;
pub mod diff;
pub mod error;
pub mod export;
pub mod files;
pub mod jobs;
pub mod local;
pub mod models;

use serde::{Deserialize, Serialize};

pub use api_client::SyncApiClient;
pub use config::{SyncConfig, SyncConfigBuilder};
pub use database::{ContextExport, DatabaseExport, DatabaseSyncService};
pub use diff::{ContentDiffCalculator, compute_content_hash};
pub use error::{SyncError, SyncResult};
pub use export::{escape_yaml, export_content_to_file, generate_content_markdown};
pub use files::{
    FileBundle, FileDiffStatus, FileEntry, FileManifest, FileSyncService, PullDownload,
    SyncDiffEntry, SyncDiffResult,
};
pub use jobs::{AccessControlSyncJob, ContentSyncJob};
pub use local::{AccessControlLocalSync, ContentDiffEntry, ContentLocalSync};
pub use models::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, LocalSyncDirection,
    LocalSyncResult,
};
pub use result::{SyncOpState, SyncOperationResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    Push,
    Pull,
}

#[derive(Debug)]
pub struct SyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl SyncService {
    pub fn new(config: SyncConfig) -> SyncResult<Self> {
        let api_client = SyncApiClient::new(&config.api_url, &config.api_token)?
            .with_direct_sync(config.hostname.clone());
        Ok(Self { config, api_client })
    }

    pub async fn sync_files(&self) -> SyncResult<SyncOperationResult> {
        let service = FileSyncService::new(self.config.clone(), self.api_client.clone());
        service.sync().await
    }

    pub async fn sync_database(&self) -> SyncResult<SyncOperationResult> {
        let local_db_url = self.config.local_database_url.as_ref().ok_or_else(|| {
            SyncError::MissingConfig("local_database_url not configured".to_owned())
        })?;

        let cloud_db_url = self
            .api_client
            .get_database_url(&self.config.tenant_id)
            .await
            .map_err(|e| SyncError::ApiError {
                status: 500,
                message: format!("Failed to get cloud database URL: {e}"),
            })?;

        let service = DatabaseSyncService::new(
            self.config.direction,
            self.config.dry_run,
            local_db_url,
            &cloud_db_url,
        );

        service.sync().await
    }

    pub async fn sync_all(&self) -> SyncResult<Vec<SyncOperationResult>> {
        let mut results = Vec::new();

        let files_result = self.sync_files().await?;
        results.push(files_result);

        match self.sync_database().await {
            Ok(db_result) => results.push(db_result),
            Err(e) => results.push(database_failure_result(&e)),
        }

        Ok(results)
    }
}

fn database_failure_result(error: &SyncError) -> SyncOperationResult {
    tracing::warn!(error = %error, "Database sync failed");
    let (state, items_synced) = match error {
        SyncError::MissingConfig(_) => (SyncOpState::NotStarted, 0),
        SyncError::PartialImport {
            completed, total, ..
        } => (
            SyncOpState::Partial {
                completed: *completed,
                total: *total,
            },
            *completed,
        ),
        _ => (SyncOpState::Failed, 0),
    };
    SyncOperationResult {
        operation: "database".to_owned(),
        success: false,
        items_synced,
        items_skipped: 0,
        errors: vec![error.to_string()],
        details: None,
        state,
    }
}
