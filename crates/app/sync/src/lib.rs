#![allow(
    clippy::unused_async,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::missing_const_for_fn,
    clippy::clone_on_ref_ptr,
    clippy::items_after_statements,
    clippy::map_unwrap_or,
    clippy::manual_let_else,
    clippy::option_if_let_else,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::redundant_closure_for_method_calls,
    clippy::unnecessary_wraps,
    clippy::if_not_else,
    clippy::unused_self,
    clippy::single_match_else
)]

pub mod api_client;
pub mod crate_deploy;
pub mod database;
pub mod diff;
pub mod error;
pub mod export;
pub mod files;
pub mod local;
pub mod models;

use serde::{Deserialize, Serialize};

pub use api_client::SyncApiClient;
pub use crate_deploy::CrateDeployService;
pub use database::{ContextExport, DatabaseExport, DatabaseSyncService, SkillExport};
pub use diff::{compute_content_hash, ContentDiffCalculator, SkillsDiffCalculator};
pub use error::{SyncError, SyncResult};
pub use export::{
    escape_yaml, export_content_to_file, export_skill_to_disk, generate_content_markdown,
    generate_skill_config, generate_skill_markdown,
};
pub use files::{FileBundle, FileEntry, FileManifest, FileSyncService};
pub use local::{ContentDiffEntry, ContentLocalSync, SkillsLocalSync};
pub use models::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, DiskSkill, LocalSyncDirection,
    LocalSyncResult, SkillDiffItem, SkillsDiffResult,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    Push,
    Pull,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub direction: SyncDirection,
    pub dry_run: bool,
    pub verbose: bool,
    pub tenant_id: String,
    pub api_url: String,
    pub api_token: String,
    pub services_path: String,
    pub database_url: Option<String>,
}

#[derive(Debug)]
pub struct SyncConfigBuilder {
    direction: SyncDirection,
    dry_run: bool,
    verbose: bool,
    tenant_id: String,
    api_url: String,
    api_token: String,
    services_path: String,
    database_url: Option<String>,
}

impl SyncConfigBuilder {
    pub fn new(
        tenant_id: impl Into<String>,
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
            database_url: None,
        }
    }

    pub const fn with_direction(mut self, direction: SyncDirection) -> Self {
        self.direction = direction;
        self
    }

    pub const fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_database_url(mut self, database_url: impl Into<String>) -> Self {
        self.database_url = Some(database_url.into());
        self
    }

    pub fn build(self) -> SyncConfig {
        SyncConfig {
            direction: self.direction,
            dry_run: self.dry_run,
            verbose: self.verbose,
            tenant_id: self.tenant_id,
            api_url: self.api_url,
            api_token: self.api_token,
            services_path: self.services_path,
            database_url: self.database_url,
        }
    }
}

impl SyncConfig {
    pub fn builder(
        tenant_id: impl Into<String>,
        api_url: impl Into<String>,
        api_token: impl Into<String>,
        services_path: impl Into<String>,
    ) -> SyncConfigBuilder {
        SyncConfigBuilder::new(tenant_id, api_url, api_token, services_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOperationResult {
    pub operation: String,
    pub success: bool,
    pub items_synced: usize,
    pub items_skipped: usize,
    pub errors: Vec<String>,
    pub details: Option<serde_json::Value>,
}

impl SyncOperationResult {
    pub fn success(operation: &str, items_synced: usize) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced,
            items_skipped: 0,
            errors: vec![],
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn dry_run(operation: &str, items_skipped: usize, details: serde_json::Value) -> Self {
        Self {
            operation: operation.to_string(),
            success: true,
            items_synced: 0,
            items_skipped,
            errors: vec![],
            details: Some(details),
        }
    }
}

#[derive(Debug)]
pub struct SyncService {
    config: SyncConfig,
    api_client: SyncApiClient,
}

impl SyncService {
    pub fn new(config: SyncConfig) -> Self {
        let api_client = SyncApiClient::new(&config.api_url, &config.api_token);
        Self { config, api_client }
    }

    pub async fn sync_files(&self) -> SyncResult<SyncOperationResult> {
        let service = FileSyncService::new(self.config.clone(), self.api_client.clone());
        service.sync().await
    }

    pub async fn sync_database(&self) -> SyncResult<SyncOperationResult> {
        let database_url = self
            .config
            .database_url
            .as_ref()
            .ok_or(SyncError::DatabaseUrlMissing)?;
        let service =
            DatabaseSyncService::new(self.config.clone(), self.api_client.clone(), database_url);
        service.sync().await
    }

    pub async fn deploy_crate(
        &self,
        skip_build: bool,
        tag: Option<String>,
    ) -> SyncResult<SyncOperationResult> {
        let service = CrateDeployService::new(self.config.clone(), self.api_client.clone());
        service.deploy(skip_build, tag).await
    }

    pub async fn sync_all(&self) -> SyncResult<Vec<SyncOperationResult>> {
        let mut results = vec![];

        results.push(self.sync_files().await?);

        if self.config.database_url.is_some() {
            results.push(self.sync_database().await?);
        }

        if self.config.direction == SyncDirection::Push {
            results.push(self.deploy_crate(false, None).await?);
        }

        Ok(results)
    }
}
