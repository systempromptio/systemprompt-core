//! Cloud sync orchestration for systemprompt.io.
//!
//! Drives push/pull of files, agents, skills, content, and database state
//! between a local systemprompt project and the systemprompt cloud (or a
//! self-hosted tenant in direct-sync mode).
//!
//! # Public surface
//!
//! - [`SyncService`], [`SyncConfig`], [`SyncConfigBuilder`] — high-level
//!   façade that wires everything together for `cloud sync` commands.
//! - [`SyncApiClient`] — low-level HTTP client for the cloud API.
//! - [`AgentsLocalSync`], [`SkillsLocalSync`], [`ContentLocalSync`] — disk ↔
//!   database sync for each domain.
//! - [`AgentsDiffCalculator`], [`SkillsDiffCalculator`],
//!   [`ContentDiffCalculator`] — pure diff computation.
//! - [`SyncError`] / [`SyncResult`] — typed error returned by every public
//!   function in this crate.
//!
//! # Feature flags
//!
//! This crate has no Cargo features.
//!
//! All public items are doc-commented; module-level `//!` docs explain
//! responsibility boundaries.

pub mod api_client;
pub mod crate_deploy;
pub mod database;
pub mod diff;
pub mod error;
pub mod export;
mod file_bundler;
pub mod files;
pub mod jobs;
pub mod local;
pub mod models;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;

pub use api_client::SyncApiClient;
pub use database::{ContextExport, DatabaseExport, DatabaseSyncService, SkillExport};
pub use diff::{
    AgentsDiffCalculator, ContentDiffCalculator, SkillsDiffCalculator, compute_content_hash,
};
pub use error::{SyncError, SyncResult};
pub use export::{
    escape_yaml, export_agent_to_disk, export_content_to_file, export_skill_to_disk,
    generate_agent_config, generate_agent_system_prompt, generate_content_markdown,
    generate_skill_config, generate_skill_markdown,
};
pub use files::{
    FileBundle, FileDiffStatus, FileEntry, FileManifest, FileSyncService, PullDownload,
    SyncDiffEntry, SyncDiffResult,
};
pub use jobs::ContentSyncJob;
pub use local::{AgentsLocalSync, ContentDiffEntry, ContentLocalSync, SkillsLocalSync};
pub use models::{
    AgentDiffItem, AgentsDiffResult, ContentDiffItem, ContentDiffResult, DiffStatus, DiskAgent,
    DiskContent, DiskSkill, LocalSyncDirection, LocalSyncResult, SkillDiffItem, SkillsDiffResult,
};

/// Direction of a sync operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    /// Push local state to the cloud / tenant.
    Push,
    /// Pull cloud / tenant state into the local working tree.
    Pull,
}

/// Fully-resolved sync configuration. Build one with [`SyncConfigBuilder`].
#[derive(Clone, Debug)]
pub struct SyncConfig {
    /// Push or Pull.
    pub direction: SyncDirection,
    /// When `true`, no remote calls are made — only the diff is reported.
    pub dry_run: bool,
    /// Emit verbose progress logs.
    pub verbose: bool,
    /// Tenant identifier the sync is targeted at.
    pub tenant_id: TenantId,
    /// Base URL of the cloud API.
    pub api_url: String,
    /// Bearer token for the cloud API.
    pub api_token: String,
    /// Local services directory (`services/` under the project root).
    pub services_path: String,
    /// Optional direct-sync hostname (skip the cloud relay).
    pub hostname: Option<String>,
    /// Optional direct-sync bearer token.
    pub sync_token: Option<String>,
    /// Optional local Postgres URL for database sync.
    pub local_database_url: Option<String>,
}

/// Builder for [`SyncConfig`] — supplies sane defaults for most fields and
/// requires only the four mandatory positional arguments.
#[derive(Debug)]
pub struct SyncConfigBuilder {
    direction: SyncDirection,
    dry_run: bool,
    verbose: bool,
    tenant_id: TenantId,
    api_url: String,
    api_token: String,
    services_path: String,
    hostname: Option<String>,
    sync_token: Option<String>,
    local_database_url: Option<String>,
}

impl SyncConfigBuilder {
    /// Construct a builder with the four mandatory inputs and default flags
    /// (`Push`, no dry-run, no verbose, no direct-sync, no local db url).
    pub fn new(
        tenant_id: impl Into<TenantId>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> Self {
        Self {
            direction: SyncDirection::Push,
            dry_run: false,
            verbose: false,
            tenant_id: tenant_id.into(),
            api_url: api_url.into(),
            api_token: api_token.into(),
            services_path: services_path.into(),
            hostname: None,
            sync_token: None,
            local_database_url: None,
        }
    }

    /// Override the default `Push` direction.
    pub const fn with_direction(mut self, direction: SyncDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Toggle dry-run mode (no remote effects).
    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Toggle verbose progress logging.
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set (or unset) the direct-sync hostname.
    pub fn with_hostname(mut self, hostname: Option<String>) -> Self {
        self.hostname = hostname;
        self
    }

    /// Set (or unset) the direct-sync bearer token.
    pub fn with_sync_token(mut self, sync_token: Option<String>) -> Self {
        self.sync_token = sync_token;
        self
    }

    /// Configure the local database URL used by `sync_database`.
    pub fn with_local_database_url(mut self, url: impl Into<String>) -> Self {
        self.local_database_url = Some(url.into());
        self
    }

    /// Materialise the [`SyncConfig`].
    pub fn build(self) -> SyncConfig {
        SyncConfig {
            direction: self.direction,
            dry_run: self.dry_run,
            verbose: self.verbose,
            tenant_id: self.tenant_id,
            api_url: self.api_url,
            api_token: self.api_token,
            services_path: self.services_path,
            hostname: self.hostname,
            sync_token: self.sync_token,
            local_database_url: self.local_database_url,
        }
    }
}

impl SyncConfig {
    /// Convenience entry point that delegates to
    /// [`SyncConfigBuilder::new`].
    pub fn builder(
        tenant_id: impl Into<TenantId>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> SyncConfigBuilder {
        SyncConfigBuilder::new(tenant_id, api_url, api_token, services_path)
    }
}

/// Per-operation completion state. Distinguishes "didn't run" from "ran but
/// failed" from "ran partially" for downstream reporting.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyncOpState {
    /// The operation never started (e.g. missing prerequisite config).
    NotStarted,
    /// The operation processed `completed` of `total` items before stopping.
    Partial {
        /// Items successfully processed.
        completed: usize,
        /// Total items the operation attempted.
        total: usize,
    },
    /// The operation finished without error.
    #[default]
    Completed,
    /// The operation failed outright.
    Failed,
}

/// Summary of a single sync operation, suitable for serialisation back to the
/// CLI / API caller.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncOperationResult {
    /// Operation label (e.g. `files`, `database`).
    pub operation: String,
    /// `true` when the operation succeeded.
    pub success: bool,
    /// Items written or imported.
    pub items_synced: usize,
    /// Items skipped (orphans not deleted, modified files not overwritten).
    pub items_skipped: usize,
    /// Display strings of any errors encountered.
    pub errors: Vec<String>,
    /// Optional structured details emitted by the operation.
    pub details: Option<serde_json::Value>,
    /// Completion state — distinguishes failure modes.
    #[serde(default)]
    pub state: SyncOpState,
}

impl SyncOperationResult {
    /// Build a `Completed` success result with the given operation label.
    pub fn success(operation: &str, items_synced: usize) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced,
            items_skipped: 0,
            errors: vec![],
            details: None,
            state: SyncOpState::Completed,
        }
    }

    /// Attach structured details.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Build a `Completed` dry-run result that reports skipped items and the
    /// computed diff under `details`.
    pub fn dry_run(operation: &str, items_skipped: usize, details: serde_json::Value) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced: 0,
            items_skipped,
            errors: vec![],
            details: Some(details),
            state: SyncOpState::Completed,
        }
    }
}

/// High-level façade combining the supplied [`SyncConfig`] with a
/// [`SyncApiClient`] and exposing convenience methods for the typical CLI
/// `cloud sync` flows.
#[derive(Debug)]
pub struct SyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl SyncService {
    /// Construct a service wrapping `config`. Builds the underlying HTTP
    /// client and applies direct-sync settings if present.
    pub fn new(config: SyncConfig) -> SyncResult<Self> {
        let api_client = SyncApiClient::new(&config.api_url, &config.api_token)?
            .with_direct_sync(config.hostname.clone(), config.sync_token.clone());
        Ok(Self { config, api_client })
    }

    /// Sync the on-disk `services_path` to or from the cloud (depending on
    /// `config.direction`).
    pub async fn sync_files(&self) -> SyncResult<SyncOperationResult> {
        let service = FileSyncService::new(self.config.clone(), self.api_client.clone());
        service.sync().await
    }

    /// Sync database state to or from the cloud. Requires
    /// `config.local_database_url` to be set.
    pub async fn sync_database(&self) -> SyncResult<SyncOperationResult> {
        let local_db_url = self.config.local_database_url.as_ref().ok_or_else(|| {
            SyncError::MissingConfig("local_database_url not configured".to_string())
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

    /// Run [`sync_files`](Self::sync_files) then
    /// [`sync_database`](Self::sync_database) and collect both results.
    /// Database failures are captured as a typed [`SyncOperationResult`]
    /// rather than propagated, so callers always get a per-step summary.
    pub async fn sync_all(&self) -> SyncResult<Vec<SyncOperationResult>> {
        let mut results = Vec::new();

        let files_result = self.sync_files().await?;
        results.push(files_result);

        match self.sync_database().await {
            Ok(db_result) => results.push(db_result),
            Err(e) => {
                tracing::warn!(error = %e, "Database sync failed");
                let (state, items_synced) = match &e {
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
                results.push(SyncOperationResult {
                    operation: "database".to_string(),
                    success: false,
                    items_synced,
                    items_skipped: 0,
                    errors: vec![e.to_string()],
                    details: None,
                    state,
                });
            },
        }

        Ok(results)
    }
}
